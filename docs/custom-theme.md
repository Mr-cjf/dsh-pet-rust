# 自定义皮肤指南

## 最快上手：一键生成模板

右键托盘 →「皮肤」→「新建皮肤模板…」
会自动生成一个皮肤目录，里面有：
- `theme.json`：已列好全部动画键，你只需把 webm 文件名填进去；
- `README.txt`：事件-动画对照表。

## 事件 → 动画对照表（照着填）

| 事件 | 动画键（填其中一个或多个） | 必需 |
|------|---------------------------|------|
| 空闲呼吸 | `idle_breath` | ✅ 必需 |
| 工作 | `work_cube` / `work_toycar` / `work_tap` | ✅ 至少 1 个 |
| 等待审批 | `react_scared` / `react_bow` | ✅ 至少 1 个 |
| 完成 | `click_happy` / `act_tailslap` | ✅ 至少 1 个 |
| 出错 | `act_rage` | ✅ 必需 |
| 点击 | `click_happy` / `click_shy` / `click_tsundere` | ✅ 至少 1 个 |
| 拖拽 | `drag_hang` | ✅ 必需 |
| 移动 | `move_crab` / `move_floatstep` / `move_runleft` | ✅ 至少 1 个 |
| 回合空闲 | `idle_lookaround` | 可选 |
| 待机随机动作 | `idle_lookaround` / `idle_hum` / `idle_stretch` / `idle_yawn` / `idle_squash` | 可选 |
| 随机动作池 | `act_violin` / `act_watergun` / `act_jump` / `act_caught` / `act_tailslap` / `fx_bubbles` / `react_bow` | 可选 |
| 彩蛋 | `act_spin` / `fx_bubbles` / `act_caught` | 可选 |
| 休息/起床 | `rest_sleep` / `react_wake` | 可选 |

**最少需要 8 个动画**：空闲 + 工作 + 等待 + 完成 + 出错 + 点击 + 拖拽 + 移动，各 1 个。

同一事件可以同时填写多个不同动画键（例如工作同时填写 `work_cube` 和 `work_toycar`），运行时会从该事件已填写的动画中随机选择。每个动画键的值既可以是一个 webm 文件名字符串，也可以是非空字符串数组；每次播放都会从数组随机选择。需要更多候选时，可以同时使用数组和多个不同动画键。`click_happy` 同时属于“完成”和“点击”事件，填写一次即可供两个事件使用。

## theme.json 示例

```json
{
  "name": "我的宠物",
  "animations": {
    "idle_breath": "idle.webm",
    "work_cube": ["work.webm", "work_alt.webm"],
    "work_toycar": "work_alt.webm",
    "react_scared": "attention.webm",
    "click_happy": "done.webm",
    "act_rage": "error.webm",
    "click_shy": "click.webm",
    "drag_hang": "drag.webm",
    "move_crab": "move.webm"
  }
}
```

未填的动画键继续使用内置鲸鱼娘。

## 视频要求

- 格式：webm（VP8）
- 背景：纯黑（#000000，程序自动转透明）
- 分辨率：约 360×360
- 无音频

## 使用

1. 托盘 →「皮肤」→「新建皮肤模板…」；
2. 把 webm 放进模板目录，填 `theme.json`；
3. 托盘 →「皮肤」→ 选你的皮肤，立即生效，重启保持。
