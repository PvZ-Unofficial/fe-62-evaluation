# PE 模拟器问题合集

记录时间：2026-06-07；最近一次清理：2026-08-24。

本文件只记录尚未修复、明确暂缓或待设计裁决的 PE 行为差异。已修复的代码 bug 见 `已经修改的bug合集.md`；已确认的项目取舍和非目标差异见 [机制差异.md](机制差异.md)；反编译源码与原版二进制的不一致见 `反汇编项目的错误.md`。

无尽核心机制应先参考 `../PlantsVsZombies-decompilation`，再使用 Ghidra 或 `objdump -d -Mintel --start-address/--stop-address` 复核 PvZ 1.0.0.1051 原版二进制。

原版二进制：`C:\Users\123\Desktop\Plants_Vs_Zombies_V1.0.0\Plants_Vs_Zombies_V1.0.0.1051_EN\PlantsVsZombies.exe`；SHA-256：`F9669AF338964787A3785A7895791297D599295B8BB669B0DB49443F736A1322`。

## 暂不修的 bug（world action / 铲除 API）

### W1. 世界 action 铲除不等价于原版 shovel 交互

- 状态：暂不修；影响 `world::update()` / action API 的铲除语义和 action mask 语义；普通系统内“对象死亡”路径不一定受影响。
- PE 位置：`world.cpp:89-117`、`world.cpp:120-187`、`system/plant/plant_factory.cpp:766-804`。
- 原版源码：`../PlantsVsZombies-decompilation/Lawn/Board.cpp` 的 `MouseDownWithTool()`、`MouseHitTestPlant()`、`SpecialPlantHitTest()` 和 `Plant::Die()`。
- 差异说明：PE action `op == -1` 固定按 coffee/content/base 顺序直接 `plant_factory.destroy()`，`op == -2` 直接销毁 pumpkin；这不是原版 shovel 的点击命中顺序。原版 shovel 会先走特殊植物命中（南瓜圆形区域、咖啡豆等 flying plant），再按 `TOPPLANT_DIGGING_ORDER` 命中普通/底座，并且在有南瓜时不会直接铲睡莲/花盆。原版铲除香蒲且同格有南瓜时还会补种睡莲，PE action 直接 destroy 香蒲时不会补睡莲。
- objdump 复核：`MouseDownWithTool` `0x411060` 在 `0x411121-0x41113d` 调 `MouseHitTest()` 后取得命中对象，`0x4111bd-0x4111df` 对 shovel 命中的 plant 调 `Plant::Die()`；`0x4111e4-0x41123a` 检查 seed `0x2b`（cattail）和同格 pumpkin 后调用 `NewPlant(..., 0x10)` 补 lily pad。
- 影响：使用 action API 做阵型编辑、强化学习动作或脚本铲除时，南瓜/底座/咖啡豆/香蒲+南瓜等叠层格的结果会偏离原版。

## 暂不修的 bug（Python 绑定 / API）

### P8. pybind 的支持边界仍需独立确认

- 状态：待设计确认；只审计最终 Legacy PE C++ 公共 API，不再以已裁撤的 RSVZ bridge/facade 为补齐目标。
- PE 位置：`pybind.cpp`、`world.h`。
- 差异说明：当前 PE 仓库不再包含 Rust bridge、timing/direct/helper facade；确定性 setup、组合策略和 Rust 适配已迁到 RustVsZombies。现有 pybind 暴露 `World(scene_type)`、`reset(scene_type)`、`update()`、`plant()`、`check_build()` 等 Legacy 入口；是否还漏掉应承诺给 Python 的 Legacy API，需要在 Python module 产品边界确定后另做逐项审计。
- 原版 / objdump 关系：Python binding 是 PE 辅助 API，没有原版等价函数。
- 影响：README 仍宣称该项目可作为 Python module 使用。若继续支持 pybind，应只修复经独立审计确认的 Legacy 缺口；若废弃则明确标注 unsupported，不能以此恢复已删除的 PE facade。
