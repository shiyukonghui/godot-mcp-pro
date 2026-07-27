//! Godot 类型到 JSON 的序列化
//! 兼容 godot 0.5.x API

use godot::builtin::{
    Color, NodePath, Rect2, Variant, VariantType, VarArray, Vector2, Vector3,
};

/// 将 Godot Variant 序列化为 JSON 值
pub fn serialize_variant(value: &Variant) -> serde_json::Value {
    match value.get_type() {
        VariantType::NIL => serde_json::Value::Null,
        VariantType::BOOL => serde_json::json!(value.to::<bool>()),
        VariantType::INT => serde_json::json!(value.to::<i64>()),
        VariantType::FLOAT => serde_json::json!(value.to::<f64>()),
        VariantType::STRING => serde_json::json!(value.to::<String>()),

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

        // Dictionary 简化处理: 转为字符串 (避免 keys() API 兼容问题)
        VariantType::DICTIONARY => {
            serde_json::json!(value.to::<String>())
        }

        // Array
        VariantType::ARRAY => {
            let arr = value.to::<VarArray>();
            let nil = Variant::nil();
            let items: Vec<serde_json::Value> = (0..arr.len())
                .map(|i| arr.get(i).unwrap_or(nil.clone()))
                .map(|v| serialize_variant(&v))
                .collect();
            serde_json::Value::Array(items)
        }

        VariantType::OBJECT => serde_json::json!(value.to::<String>()),
        _ => serde_json::json!(value.to::<String>()),
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
            else { Variant::from(serde_json::to_string(value).unwrap_or_default()) }
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
