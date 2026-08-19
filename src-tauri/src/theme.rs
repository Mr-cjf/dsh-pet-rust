//! 多皮肤/主题：扫描 ~/.dsh-pet/themes/<id>/theme.json，支持用户自定义动画。
//!
//! theme.json 约定：
//! {
//!   "name": "显示名",
//!   "animations": {
//!     "idle_breath": "idle.webm",
//!     "work": ["cube.webm", "toycar.webm"]
//!   }
//! }
//!
//! 每个事件（动画键或事件名）值可为单字符串或字符串数组，运行时随机选一个 webm 播放。
//! 同一事件支持多个不同动画键（旧键池）以增加随机性；事件名数组写法为新格式（优先来源）。
//! 字符串或非空字符串数组是推荐写法；空字符串/空数组会被忽略。归一化会保留非空字符串（去重保序）。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawAnimationValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnimationList(pub Vec<String>);

impl AnimationList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn from_raw(value: RawAnimationValue) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let push =
            |s: String, out: &mut Vec<String>, seen: &mut std::collections::HashSet<String>| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return;
                }
                if seen.insert(trimmed.to_string()) {
                    out.push(trimmed.to_string());
                }
            };
        match value {
            RawAnimationValue::One(s) => push(s, &mut out, &mut seen),
            RawAnimationValue::Many(items) => {
                for s in items {
                    push(s, &mut out, &mut seen);
                }
            }
        }
        Self(out)
    }
}

impl<'de> Deserialize<'de> for AnimationList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = AnimationList;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or array of strings")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                Ok(AnimationList::from_raw(RawAnimationValue::One(
                    s.to_string(),
                )))
            }

            fn visit_string<E: de::Error>(self, s: String) -> Result<Self::Value, E> {
                Ok(AnimationList::from_raw(RawAnimationValue::One(s)))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(AnimationList::default())
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(AnimationList::default())
            }

            fn visit_some<D2: Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
                d.deserialize_any(V)
            }

            fn visit_seq<A2: SeqAccess<'de>>(self, mut seq: A2) -> Result<Self::Value, A2::Error> {
                let mut items = Vec::new();
                while let Some(v) = seq.next_element::<String>()? {
                    items.push(v);
                }
                Ok(AnimationList::from_raw(RawAnimationValue::Many(items)))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl Serialize for AnimationList {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.len() == 1 {
            serializer.serialize_str(&self.0[0])
        } else {
            self.0.serialize(serializer)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(default)]
    pub animations: HashMap<String, AnimationList>,
}

pub type ThemeAnimations = HashMap<String, Vec<String>>;

#[derive(Debug, Clone, Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
}

pub fn themes_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("themes")
}

pub fn scan_themes(app: &AppHandle) -> Vec<ThemeInfo> {
    let dir = themes_dir(app);
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            let theme_json = entry.path().join("theme.json");
            let name = std::fs::read_to_string(&theme_json)
                .ok()
                .and_then(|s| serde_json::from_str::<Theme>(&s).ok())
                .map(|t| t.name)
                .unwrap_or_else(|| id.clone());
            result.push(ThemeInfo { id, name });
        }
    }
    result
}

pub fn load_theme(app: &AppHandle, id: &str) -> Option<Theme> {
    let path = themes_dir(app).join(id).join("theme.json");
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn normalized_animations(theme: &Theme) -> ThemeAnimations {
    theme
        .animations
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k.clone(), v.as_slice().to_vec()))
        .collect()
}

pub fn theme_animation_path(app: &AppHandle, id: &str, file: &str) -> Option<PathBuf> {
    let p = themes_dir(app).join(id).join(file);
    p.exists().then_some(p)
}

pub fn open_themes_dir(app: &AppHandle) {
    let dir = themes_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&dir).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
}

pub fn open_editor(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("theme-editor") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "theme-editor",
        tauri::WebviewUrl::App("index.html?editor=1".into()),
    )
    .title("皮肤编辑器")
    .inner_size(980.0, 760.0)
    .min_inner_size(760.0, 560.0)
    .resizable(true)
    .build();
}

pub const EVENT_GROUPS: &[(&str, &[&str])] = &[
    ("空闲呼吸（必需）", &["idle_breath"]),
    (
        "工作（至少 1 个）",
        &["work_cube", "work_toycar", "work_tap"],
    ),
    ("等待审批（至少 1 个）", &["react_scared", "react_bow"]),
    ("完成（至少 1 个）", &["click_happy", "act_tailslap"]),
    ("出错（必需）", &["act_rage"]),
    ("回合空闲（可选）", &["idle_lookaround"]),
    (
        "点击（至少 1 个）",
        &["click_happy", "click_shy", "click_tsundere"],
    ),
    ("拖拽（必需）", &["drag_hang"]),
    (
        "移动（至少 1 个）",
        &["move_crab", "move_floatstep", "move_runleft"],
    ),
    ("彩蛋（可选）", &["act_spin", "fx_bubbles", "act_caught"]),
    (
        "待机随机动作（可选）",
        &[
            "idle_lookaround",
            "idle_hum",
            "idle_stretch",
            "idle_yawn",
            "idle_squash",
        ],
    ),
    (
        "随机动作池（可选）",
        &[
            "act_violin",
            "act_watergun",
            "act_jump",
            "act_caught",
            "act_rage",
            "act_tailslap",
            "fx_bubbles",
            "react_bow",
        ],
    ),
    ("休息/起床（可选）", &["rest_sleep", "react_wake"]),
];

pub fn create_template_theme(app: &AppHandle) -> Option<ThemeInfo> {
    let dir = themes_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    let seq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() % 100000)
        .unwrap_or(0);
    let id = format!("my-pet-{seq}");
    let path = dir.join(&id);
    std::fs::create_dir_all(&path).ok()?;

    let theme = serde_json::json!({
        "name": id,
        "animations": {
            "idle_breath": "idle.webm",
            "work": ["work_cube.webm", "work_toycar.webm", "work_tap.webm"],
            "react_scared": "",
            "react_bow": "react_bow.webm",
            "click_happy": ["click_happy.webm"],
            "act_tailslap": "act_tailslap.webm",
            "act_rage": "act_rage.webm",
            "click_shy": "click_shy.webm",
            "click_tsundere": "click_tsundere.webm",
            "drag_hang": "drag_hang.webm",
            "move_crab": "move_crab.webm",
            "move_floatstep": "",
            "move_runleft": "move_runleft.webm"
        }
    });
    std::fs::write(
        path.join("theme.json"),
        serde_json::to_string_pretty(&theme).unwrap(),
    )
    .ok()?;

    let mut readme = String::from(
        r#"自定义皮肤动画对照表
========================

把动画文件（webm）放进本目录，然后在 theme.json 里把对应文件名填到动画键后面。
animations 的每个键值可以是单字符串或字符串数组：
  单字符串："idle_breath": "idle.webm"
  字符串数组："work": ["work_cube.webm", "work_toycar.webm"]
数组每次播放都会随机选择；空字符串和空数组会被忽略。
同一个事件也可以同时填写多个不同动画键（例如 work_cube、work_toycar），运行时会从全部候选中随机选择。
事件名（如 work）会把同一数组提供给该事件的预置动画键；动画键（如 work_cube）只覆盖单个键。
click_happy 同时属于“完成”和“点击”事件，填写一次即可供两个事件使用。

至少需要 8 个动画（每个“必需/至少1个”分组各 1 个）：

"#,
    );
    for (label, keys) in EVENT_GROUPS {
        readme.push_str(&format!(
            "- {}：{}
",
            label,
            keys.join(" / ")
        ));
    }
    readme.push_str(
        r#"
说明：
- webm（VP8），纯黑背景，约 360x360，无音频；
- 填好 theme.json 后，右键托盘 → 皮肤 → 选择本皮肤即可生效。
"#,
    );
    std::fs::write(path.join("README.txt"), readme).ok()?;

    Some(ThemeInfo {
        id: id.clone(),
        name: id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_single_string() {
        let json = r#"{
            "name": "t",
            "animations": { "idle_breath": "idle.webm" }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        assert_eq!(
            t.animations.get("idle_breath").unwrap().0,
            vec!["idle.webm"]
        );
    }

    #[test]
    fn deserialize_string_array() {
        let json = r#"{
            "name": "t",
            "animations": { "work": ["cube.webm", "toycar.webm"] }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        assert_eq!(
            t.animations.get("work").unwrap().0,
            vec!["cube.webm", "toycar.webm"]
        );
    }

    #[test]
    fn deserialize_dedup_and_trim() {
        let json = r#"{
            "name": "t",
            "animations": {
                "work": ["a.webm", "  ", "", "a.webm", "b.webm", " b.webm "]
            }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        assert_eq!(
            t.animations.get("work").unwrap().0,
            vec!["a.webm", "b.webm"]
        );
    }

    #[test]
    fn deserialize_empty_string_and_empty_array() {
        let json = r#"{
            "name": "t",
            "animations": {
                "a": "",
                "b": [],
                "c": ["", "   "]
            }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        assert!(t.animations.get("a").map(|v| v.is_empty()).unwrap_or(true));
        assert!(t.animations.get("b").map(|v| v.is_empty()).unwrap_or(true));
        assert!(t.animations.get("c").map(|v| v.is_empty()).unwrap_or(true));
    }

    #[test]
    fn normalized_animations_filters_empty() {
        let json = r#"{
            "name": "t",
            "animations": {
                "work": ["cube.webm", "toycar.webm"],
                "idle": "",
                "rest": []
            }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        let n = normalized_animations(&t);
        assert_eq!(
            n.get("work").unwrap(),
            &vec!["cube.webm".to_string(), "toycar.webm".to_string()]
        );
        assert!(!n.contains_key("idle"));
        assert!(!n.contains_key("rest"));
    }

    #[test]
    fn normalized_animations_keeps_multiple_event_keys() {
        let json = r#"{
            "name": "t",
            "animations": {
                "work_cube": "cube.webm",
                "work_toycar": ["toycar.webm", "toycar-alt.webm"],
                "work_tap": "tap.webm"
            }
        }"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        let n = normalized_animations(&t);
        assert_eq!(n.len(), 3);
        assert_eq!(n["work_cube"], vec!["cube.webm"]);
        assert_eq!(n["work_toycar"], vec!["toycar.webm", "toycar-alt.webm"]);
        assert_eq!(n["work_tap"], vec!["tap.webm"]);
    }

    #[test]
    fn serialize_round_trip_keeps_values() {
        let json = r#"{"name":"t","animations":{"idle_breath":"idle.webm","work":["a","b"]}}"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        let out = serde_json::to_string(&t).unwrap();
        let t2: Theme = serde_json::from_str(&out).unwrap();
        assert_eq!(
            t2.animations.get("idle_breath").unwrap().0,
            vec!["idle.webm"]
        );
        assert_eq!(t2.animations.get("work").unwrap().0, vec!["a", "b"]);
    }
}
