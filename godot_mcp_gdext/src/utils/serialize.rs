//! Godot 类型到 JSON 的序列化
//! 兼容 godot 0.5.x API

use godot::builtin::{
    AnyArray, Color, NodePath, Rect2, StringName, Variant, VariantType, VarArray, Vector2,
    Vector3,
};
use godot::obj::Gd;

/// 将 Godot Variant 序列化为 JSON 值
pub fn serialize_variant(value: &Variant) -> serde_json::Value {
    match value.get_type() {
        VariantType::NIL => serde_json::Value::Null,
        VariantType::BOOL => serde_json::json!(value.to::<bool>()),
        VariantType::INT => serde_json::json!(value.to::<i64>()),
        VariantType::FLOAT => serde_json::json!(value.to::<f64>()),
        VariantType::STRING => serde_json::json!(value.to::<String>()),
        VariantType::STRING_NAME => serde_json::json!(value.to::<StringName>().to_string()),

        VariantType::VECTOR2 => {
            let v = value.to::<Vector2>();
            serde_json::json!({"x": v.x, "y": v.y})
        }
        VariantType::VECTOR2I => {
            let v = value.to::<godot::builtin::Vector2i>();
            serde_json::json!({"x": v.x, "y": v.y})
        }
        VariantType::VECTOR3 => {
            let v = value.to::<Vector3>();
            serde_json::json!({"x": v.x, "y": v.y, "z": v.z})
        }
        VariantType::VECTOR3I => {
            let v = value.to::<godot::builtin::Vector3i>();
            serde_json::json!({"x": v.x, "y": v.y, "z": v.z})
        }

        VariantType::COLOR => {
            let c = value.to::<Color>();
            serde_json::json!({
                "r": c.r, "g": c.g, "b": c.b, "a": c.a,
                "html": format!("#{:02x}{:02x}{:02x}", (c.r*255.0) as u8, (c.g*255.0) as u8, (c.b*255.0) as u8)
            })
        }

        VariantType::RECT2 => {
            let r = value.to::<Rect2>();
            serde_json::json!({"x": r.position.x, "y": r.position.y, "width": r.size.x, "height": r.size.y})
        }

        VariantType::NODE_PATH => {
            serde_json::json!(value.to::<NodePath>().to_string())
        }

        // Dictionary：递归序列化为结构化 JSON 对象
        VariantType::DICTIONARY => {
            let dict: godot::builtin::Dictionary<Variant, Variant> = value.to();
            let mut map = serde_json::Map::new();
            for (key, val) in dict.iter_shared() {
                let key_str = variant_to_key(&key);
                map.insert(key_str, serialize_variant(&val));
            }
            serde_json::Value::Object(map)
        }

        // Array：使用 AnyArray 安全读取类型化数组
        VariantType::ARRAY => {
            let arr = value.to::<AnyArray>();
            let nil = Variant::nil();
            let items: Vec<serde_json::Value> = (0..arr.len())
                .map(|i| arr.get(i).unwrap_or(nil.clone()))
                .map(|v| serialize_variant(&v))
                .collect();
            serde_json::Value::Array(items)
        }

        // Object：获取真实类名和 Node 路径
        VariantType::OBJECT => {
            let instance_id = value.object_id_unchecked().map(|id| id.to_string());
            // 尝试转换为 Gd<Object> 获取类名
            match value.try_to::<Gd<godot::classes::Object>>() {
                Ok(obj) => {
                    let cn = obj.get_class().to_string();
                    // 尝试转换为 Node 以获取场景路径
                    match obj.try_cast::<godot::classes::Node>() {
                        Ok(node) => {
                            let path = node.get_path().to_string();
                            serde_json::json!({
                                "type": cn,
                                "instance_id": instance_id,
                                "path": if path.is_empty() { None } else { Some(path) },
                                "value": value.stringify().to_string(),
                            })
                        }
                        Err(_) => serde_json::json!({
                            "type": cn,
                            "instance_id": instance_id,
                            "value": value.stringify().to_string(),
                        }),
                    }
                }
                Err(_) => serde_json::json!({
                    "type": "Object",
                    "instance_id": instance_id,
                    "value": value.stringify().to_string(),
                }),
            }
        }

        // Callable：返回方法名和目标对象信息
        VariantType::CALLABLE => {
            let c = value.to::<godot::builtin::Callable>();
            let method = c.method_name().map(|n| n.to_string()).unwrap_or_default();
            // 获取目标对象（可能为空）
            let target_info = c.object().map(|obj: Gd<godot::classes::Object>| {
                serde_json::json!({
                    "class": obj.get_class().to_string(),
                })
            });
            serde_json::json!({
                "type": "Callable",
                "method": method,
                "target": target_info,
            })
        }

        // Signal：返回信号名
        VariantType::SIGNAL => {
            let s = value.to::<godot::builtin::Signal>();
            serde_json::json!({
                "type": "Signal",
                "name": s.name().to_string(),
                "value": value.stringify().to_string(),
            })
        }

        // RID：Godot 内部资源标识
        VariantType::RID => {
            serde_json::json!({
                "type": "RID",
                "value": value.stringify().to_string(),
            })
        }

        // Transform2D：2×3 仿射矩阵
        VariantType::TRANSFORM2D => {
            let t = value.to::<godot::builtin::Transform2D>();
            serde_json::json!({
                "x": {"x": t.a.x, "y": t.a.y},
                "y": {"x": t.b.x, "y": t.b.y},
                "origin": {"x": t.origin.x, "y": t.origin.y},
            })
        }

        // Basis：3×3 旋转/缩放矩阵（rows 字段是 [Vector3; 3]）
        VariantType::BASIS => {
            let b: godot::builtin::Basis = value.to();
            serde_json::json!({
                "x": {"x": b.rows[0].x, "y": b.rows[0].y, "z": b.rows[0].z},
                "y": {"x": b.rows[1].x, "y": b.rows[1].y, "z": b.rows[1].z},
                "z": {"x": b.rows[2].x, "y": b.rows[2].y, "z": b.rows[2].z},
            })
        }

        // AABB：轴对齐包围盒
        VariantType::AABB => {
            let a: godot::builtin::Aabb = value.to();
            serde_json::json!({
                "position": {"x": a.position.x, "y": a.position.y, "z": a.position.z},
                "size": {"x": a.size.x, "y": a.size.y, "z": a.size.z},
            })
        }

        // Plane：平面（法线 + 距离）
        VariantType::PLANE => {
            let p: godot::builtin::Plane = value.to();
            serde_json::json!({
                "normal": {"x": p.normal.x, "y": p.normal.y, "z": p.normal.z},
                "d": p.d,
            })
        }

        // Projection：4×4 投影矩阵（cols 字段是 [Vector4; 4]）
        VariantType::PROJECTION => {
            let p: godot::builtin::Projection = value.to();
            serde_json::json!({
                "x": {"x": p.cols[0].x, "y": p.cols[0].y, "z": p.cols[0].z, "w": p.cols[0].w},
                "y": {"x": p.cols[1].x, "y": p.cols[1].y, "z": p.cols[1].z, "w": p.cols[1].w},
                "z": {"x": p.cols[2].x, "y": p.cols[2].y, "z": p.cols[2].z, "w": p.cols[2].w},
                "w": {"x": p.cols[3].x, "y": p.cols[3].y, "z": p.cols[3].z, "w": p.cols[3].w},
            })
        }

        // Quaternion
        VariantType::QUATERNION => {
            let q: godot::builtin::Quaternion = value.to();
            serde_json::json!({"x": q.x, "y": q.y, "z": q.z, "w": q.w})
        }

        // PackedArray 各种类型：转换为 JSON 数组
        VariantType::PACKED_BYTE_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedByteArray>()
                .as_slice().iter().map(|&b| serde_json::json!(b)).collect();
            serde_json::Value::Array(arr)
        }
        VariantType::PACKED_INT32_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedInt32Array>()
                .as_slice().iter().map(|&i| serde_json::json!(i)).collect();
            serde_json::Value::Array(arr)
        }
        VariantType::PACKED_INT64_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedInt64Array>()
                .as_slice().iter().map(|&i| serde_json::json!(i)).collect();
            serde_json::Value::Array(arr)
        }
        VariantType::PACKED_FLOAT32_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedFloat32Array>()
                .as_slice().iter().map(|&f| serde_json::json!(f as f64)).collect();
            serde_json::Value::Array(arr)
        }
        VariantType::PACKED_FLOAT64_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedFloat64Array>()
                .as_slice().iter().map(|&f| serde_json::json!(f)).collect();
            serde_json::Value::Array(arr)
        }
        // PackedStringArray：GString 不实现 Serialize，手动转 String
        VariantType::PACKED_STRING_ARRAY => {
            let arr: Vec<serde_json::Value> = value.to::<godot::builtin::PackedStringArray>()
                .as_slice().iter().map(|s| serde_json::json!(s.to_string())).collect();
            serde_json::Value::Array(arr)
        }
        VariantType::PACKED_VECTOR2_ARRAY => {
            let arr = value.to::<godot::builtin::PackedVector2Array>();
            let items: Vec<serde_json::Value> = arr.as_slice().iter()
                .map(|v| serde_json::json!({"x": v.x, "y": v.y}))
                .collect();
            serde_json::Value::Array(items)
        }
        VariantType::PACKED_VECTOR3_ARRAY => {
            let arr = value.to::<godot::builtin::PackedVector3Array>();
            let items: Vec<serde_json::Value> = arr.as_slice().iter()
                .map(|v| serde_json::json!({"x": v.x, "y": v.y, "z": v.z}))
                .collect();
            serde_json::Value::Array(items)
        }
        VariantType::PACKED_COLOR_ARRAY => {
            let arr = value.to::<godot::builtin::PackedColorArray>();
            let items: Vec<serde_json::Value> = arr.as_slice().iter()
                .map(|c| serde_json::json!({"r": c.r, "g": c.g, "b": c.b, "a": c.a}))
                .collect();
            serde_json::Value::Array(items)
        }
        // 未显式支持的内建类型使用 Godot 自身的字符串化接口
        _ => serde_json::json!(value.stringify().to_string()),
    }
}

/// 将 Variant 键转换为 JSON 对象的字符串键
fn variant_to_key(key: &Variant) -> String {
    match key.get_type() {
        VariantType::STRING => key.to::<String>(),
        VariantType::STRING_NAME => key.to::<StringName>().to_string(),
        VariantType::INT => key.to::<i64>().to_string(),
        _ => key.stringify().to_string(),
    }
}

/// 从 JSON Value 解析为 Godot Variant (用于设置属性)
pub fn parse_value_for_property(value: &serde_json::Value) -> Variant {
    match value {
        serde_json::Value::Null => Variant::nil(),
        serde_json::Value::Bool(b) => Variant::from(*b),
        serde_json::Value::Number(n) => {
            if n.is_f64() { Variant::from(n.as_f64().unwrap_or(0.0) as f32) }
            else { Variant::from(n.as_i64().unwrap_or(0)) }
        }
        serde_json::Value::String(s) => {
            if let Some(v) = parse_godot_string(s) { v }
            else { Variant::from(s.clone()) }
        }
        serde_json::Value::Array(arr) => {
            let mut gd_arr = VarArray::new();
            for v in arr { gd_arr.push(&parse_value_for_property(v)); }
            Variant::from(gd_arr)
        }
        serde_json::Value::Object(obj) => {
            if let Some(v) = try_parse_godot_struct(obj) { v }
            else {
                // 普通 JSON Object 转换为 Godot Dictionary，保留结构
                let mut dict = godot::builtin::Dictionary::<Variant, Variant>::new();
                for (k, v) in obj {
                    dict.set(&Variant::from(k.clone()), &parse_value_for_property(v));
                }
                Variant::from(dict)
            }
        }
    }
}

fn parse_godot_string(s: &str) -> Option<Variant> {
    let s = s.trim();
    if s.starts_with('#') {
        return Color::from_html(s).map(|c| Variant::from(c));
    }
    let nums = |s: &str| -> Vec<f64> {
        let cleaned = s.trim_start_matches(|c: char| c.is_alphabetic() || c == '(').trim_end_matches(')');
        cleaned.split(',').filter_map(|p| p.trim().parse().ok()).collect()
    };
    if s.starts_with("Vector2(") || s.starts_with("Vector2i(") {
        let n = nums(s);
        if n.len() >= 2 { return Some(Variant::from(Vector2::new(n[0] as f32, n[1] as f32))); }
    }
    if s.starts_with("Vector3(") || s.starts_with("Vector3i(") {
        let n = nums(s);
        if n.len() >= 3 { return Some(Variant::from(Vector3::new(n[0] as f32, n[1] as f32, n[2] as f32))); }
    }
    None
}

fn try_parse_godot_struct(obj: &serde_json::Map<String, serde_json::Value>) -> Option<Variant> {
    let has_xyz = obj.contains_key("x") || obj.contains_key("y") || obj.contains_key("z");
    let has_rgba = obj.contains_key("r") || obj.contains_key("g") || obj.contains_key("b");

    if has_xyz && obj.contains_key("z") {
        return Some(Variant::from(Vector3::new(gf(obj,"x"), gf(obj,"y"), gf(obj,"z"))));
    }
    if has_rgba {
        return Some(Variant::from(Color::from_rgba(gf(obj,"r"), gf(obj,"g"), gf(obj,"b"), gf(obj,"a"))));
    }
    if has_xyz {
        return Some(Variant::from(Vector2::new(gf(obj,"x"), gf(obj,"y"))));
    }
    None
}

fn gf(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> f32 {
    obj.get(key).and_then(|v| v.as_f64()).unwrap_or(if key == "a" { 1.0 } else { 0.0 }) as f32
}
