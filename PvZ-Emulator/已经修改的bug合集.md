# 已经修改的 bug 合集

## 1. 机枪射手升级基础植物判定错误

- 问题描述：机枪射手的高级植物判定把目标基础植物误判为 `gatling_pea`，应当要求目标是 `repeater`。该提交修正的是机枪射手升级的基础类型判断，不代表所有升级植物种植流程问题都已完整修复。
- 修改文件：`system/plant/plant_factory.cpp`
- 修改提交号：`75f45b6a1565417216cc6f9eb9fb423b0c2fbaaf`

## 2. 自然阳光倒计时重置问题

- 问题描述：自然阳光倒计时为 0 时没有先重新生成倒计时，可能导致重置后自然阳光刷新节奏异常。
- 修改文件：`system/sun.cpp`
- 修改提交号：`75f45b6a1565417216cc6f9eb9fb423b0c2fbaaf`

## 3. 升级植物被普通占位判定提前拒绝

- 问题描述：升级植物在目标格已有普通植物时会先被普通占位检查拒绝，导致机枪射手、双子向日葵、忧郁菇、香蒲、冰瓜、吸金磁、玉米加农炮等升级路径无法走到高级植物判定。
- 修改文件：`system/plant/plant_factory.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 4. 玉米加农炮缺少双格南瓜头判定

- 问题描述：玉米加农炮高级植物判定只检查左右两个玉米投手和底座一致性，没有排除目标格和右侧相邻格存在南瓜头的情况。
- 修改文件：`system/plant/plant_factory.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 5. 磁力菇吸梯子距离计算错误

- 问题描述：磁力菇吸取已放置梯子时使用了 `item.col - item.col` 和 `item.row - item.row`，导致距离恒为 0，会跨图吸走梯子；同时梯子候选的距离微权重应按行差对齐原版。
- 修改文件：`system/plant/mushroom_family.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 6. 直线子弹贴边命中被误改为不命中

- 问题描述：`9f7069c` 根据反编译源码把 projectile 命中僵尸的横向重叠条件从 `>= 0` 改成 `> 0`，导致攻击框和受击框刚好相切时漏判命中。objdump 确认原版 `Projectile::FindCollisionTarget` 只在 overlap < 0 时跳过，贴边应命中；现在回退为 `>= 0`。
- 修改文件：`system/projectile/projectile_system.cpp`
- 修改提交号：`23d549f46ed505589261f8cb814248f6a2554891`

## 7. 投篮车篮球跳过低矮植物

- 问题描述：篮球寻找植物目标时复用了低矮植物排除逻辑，导致土豆雷等低矮植物不会被篮球砸中；该低矮植物过滤不应套用于篮球。
- 修改文件：`system/projectile/projectile_system.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 8. 火豌豆攻击火抗目标伤害标志错误

- 问题描述：火豌豆命中门板、梯子、冰车、投篮车等火抗目标时仍按溅射/不衰减路径处理，可能同时伤害防具和本体；现在按火抗目标取消火豌豆溅射，并整理相关判断括号避免优先级回归。
- 修改文件：`object/projectile.cpp`、`system/projectile/projectile_system.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 9. 寒冰菇水中僵尸冻结时长被覆盖

- 问题描述：寒冰菇命中水中僵尸时先设置冻结倒计时为 300，但后续独立 `if` 会继续进入陆地随机时长分支，导致水中固定 300 被覆盖。
- 修改文件：`system/damage.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 10. 火爆辣椒没有缩短冰道倒计时

- 问题描述：火爆辣椒清行后没有把本行冰道倒计时压到短倒计时，导致冰道持续时间与原版不一致。
- 修改文件：`system/damage.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 11. projectile 对象池复用保留旧 countdown

- 问题描述：projectile 槽位复用后没有清空 `countdown` 字段，新 projectile 会继承旧对象倒计时。
- 修改文件：`system/projectile/projectile_factory.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 12. griditem 对象池复用保留旧消失状态

- 问题描述：griditem 槽位复用后没有重置 `is_disappeared`，新 griditem 可能创建后立刻处于可回收状态。
- 修改文件：`system/griditem_factory.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 13. 普通僵尸复用梯子僵尸槽位后保留 accessory_2.hp

- 问题描述：普通僵尸初始化时设置第二配件类型为 none 后误清空了 `accessory_1.hp`，没有清空 `accessory_2.hp`，导致对象池复用后残留梯子等第二配件血量。
- 修改文件：`system/zombie/common_zombie.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 14. 对象 ID 缺少 DataArray generation key

- 问题描述：对象列表原先只使用槽位下标作为 ID，旧 ID 在槽位复用后可能解析到新对象；现在按原版 DataArray 风格使用 key 和 index 组合，使 `0`、陈旧或 generation 不匹配的 ID 返回空。高位 full-ID 经 `int` 传递及 `get_index()` 低 16 位语义由第 51 项补全。
- 修改文件：`object/obj_list.h`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 15. 蹦极阳光植物计数和目标优先级不一致

- 问题描述：蹦极目标选择中阳光植物计数条件写反，且计数、权重选择、抓取阶段各自手写顶层植物优先级，容易出现分歧；现在统一按咖啡豆、内容植物、南瓜、底座的目标优先级处理。
- 修改文件：`system/zombie/bungee.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 16. 投篮车会选择过近或右侧植物

- 问题描述：投篮车寻找篮球目标时没有检查植物必须在自身左侧且距离足够，导致过近或右侧植物也可能被选为目标。
- 修改文件：`system/zombie/catapult.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 17. 屋顶巨人投小鬼竖向速度下限异常

- 问题描述：屋顶场景下巨人投小鬼只调整了投掷距离，没有把最小投掷距离改为原版允许的负值，导致屋顶竖向速度参数方向异常。
- 修改文件：`system/zombie/gargantuar.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 18. 伴舞僵尸出土高度使用整数除法

- 问题描述：伴舞僵尸出土高度使用 `-4/3` 整数除法，实际变成 `-1`，出土曲线高度不符合原版线性动画。
- 修改文件：`system/zombie/backup_dancer.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 19. 进家边界使用严格小于和整数坐标

- 问题描述：进家判定使用 `int_x < threshold`，导致僵尸正好位于阈值时不判负；现在改为使用浮点位置并在 `x <= threshold` 时触发。
- 修改文件：`system/zombie/zombie_system.cpp`
- 修改提交号：`9f7069cdf01caa765fbbd144ba8a00c6339e7b21`

## 20. 裂荚射手 bug 触发条件不等价

- 问题描述：裂荚射手 bug 通过第二轮后向攻击时重新查找前向目标来模拟，导致“首轮有前向目标但第二轮前消失”漏发，以及“首轮无前向目标但第二轮才出现”误发；现在将 `split_pea_attack_flags` 作为前后头 shooting-hold 状态保留，在共享发射倒计时真正归零时清除，从而按原版状态残留条件触发。
- 修改文件：`system/plant/plant_system.cpp`、`object/plant.h`、`README.md`
- 修改提交号：`bc1e00d03e2a77234952666256ee11263ae2d8ba`

## 21. 红眼巨人漏进自然波表且旗帜波权重错误

- 问题描述：出怪候选遍历排除了 `giga_gargantuar`，导致高周目无尽中红眼即使允许也无法进入实际波表；同时红眼权重固定为 `1000`，没有在内部零基第 9/19 波（显示第 10/20 波）旗帜波使用原版 `6000` 权重。
- 修改文件：`system/spawn.cpp`、`system/spawn.h`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 22. 非火球溅射总伤害上限被误改低

- 问题描述：`4ab9544` 根据反编译源码把非火球溅射总伤害上限从 `7 * original_damage` 改成 `7 * (original_damage / 3)`，导致西瓜、冰瓜等非火球溅射在多目标时被过早压低。objdump 确认原版非火球上限是 `7 * original_damage`，火豌豆/火球上限是 `original_damage`；现在回退为 `7 * original_damage`。
- 修改文件：`system/projectile/projectile_system.cpp`
- 修改提交号：`23d549f46ed505589261f8cb814248f6a2554891`

## 23. 寒冰伤害命中一类防具时不施加减速

- 问题描述：冰系伤害被路障、铁桶、橄榄帽、矿工帽等一类防具吸收时没有触发减速，和原版 `TakeHelmDamage()` 行为不一致；现在只在实际命中一类防具时施加减速，不扩展到二类盾牌。
- 修改文件：`system/damage.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 24. 睡眠植物仍推进攻击装填

- 问题描述：睡眠、压扁、死亡或不可食用植物仍可能执行 `update_attack()`，导致睡眠蘑菇类植物装填提前推进；现在这些状态下跳过攻击装填，同时保留唤醒、死亡和被吃相关状态倒计时。本项旧说明把效果倒计时继续推进列为刻意保留；N29 后续依据 1051 早退机器码完成修复，见第 111 项。
- 修改文件：`system/plant/plant_system.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 25. 普通僵尸缺少无头核心语义

- 问题描述：可掉头僵尸本体血量低于三分之一后仍按有头状态移动和啃食，也不会进入原版无头随机自损节奏；现在复用 `is_not_dying = false` 表示无头核心状态，清除啃食和黄油，并接入现有近死自损逻辑。
- 修改文件：`system/damage.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 26. 普通死亡动画状态被立即销毁

- 问题描述：可播放死亡动画的僵尸进入 `dying` 状态后仍被立即销毁，丢失原版死亡动画保留帧和后续清理流程；现在死亡动画路径设置死亡状态后返回，由现有死亡倒计时清理。
- 修改文件：`system/damage.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 27. 小丑爆炸僵尸半径参数传反

- 问题描述：小丑爆炸调用 `take_instant_kill()` 时把圆半径和行半径传反，导致圆半径极小但跨行范围异常；现在 hypno 和非 hypno 分支都使用圆半径 `115`、行半径 `1`。
- 修改文件：`system/zombie/jack_in_the_box.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 28. 舞王啃食时仍推进召唤和舞步

- 问题描述：舞王僵尸在 `is_eating` 时仍会递减召唤倒计时、切换舞步或触发伴舞补召；现在在舞王更新入口按原版直接早退。
- 修改文件：`system/zombie/dancing.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 29. 卡片冷却可用边界早一帧

- 问题描述：种卡后直接把冷却设为 `CD_TABLE`，而本帧末又会立即递减，导致卡片比原版 `refresh_time + 1` 边界早一帧可用；现在种卡后设置为 `CD_TABLE + 1`。
- 修改文件：`system/plant/plant_factory.cpp`
- 修改提交号：`4ab9544fc5f1bdffabc9046c0378aebf1c7e1c96`

## 30. 阳光菇更新落入双子向日葵产阳逻辑

- 问题描述：阳光菇更新后缺少 `break`，会继续执行双子向日葵产阳逻辑，导致产阳倒计时额外推进；现在阳光菇只执行自身更新。
- 修改文件：`system/plant/plant_system.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 31. 海蘑菇初始化为小喷菇类型

- 问题描述：海蘑菇复用小喷菇初始化宏时传入了 `puffshroom`，导致对象类型被改成小喷菇；现在初始化后保留 `seashroom` 类型。
- 修改文件：`system/plant/plant.h`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 32. 普通射手默认攻击 flags 包含动画态僵尸

- 问题描述：普通默认植物攻击 flags 多带 `animating_zombies`，可能命中撑杆、海豚、小鬼等动画态僵尸；现在默认分支只返回地面目标，特殊植物继续显式声明额外 flags。
- 修改文件：`object/plant.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 33. 倭瓜候选目标没有按实际距离和锁定目标处理

- 问题描述：倭瓜目标选择用候选阈值替代实际重叠距离排序，且跳跃后没有优先保留已锁定目标；现在先验证锁定目标，再按实际 `-overlap` 距离选择最近候选。
- 修改文件：`system/plant/squash.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 34. reanimation 跨循环事件判断使用动画速率字段

- 问题描述：循环跨越 1.0 时的定时事件判断误用 `prev_fps` 作为上一帧进度，可能漏触发或误触发动画事件；现在使用 `prev_progress`。
- 修改文件：`object/common.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 35. 投篮车最后一发后仍可再次进入发射状态

- 问题描述：投篮车最后一发篮球后会进入 idle，并可能在 0 发篮球时再次切入发射动画；现在 idle 转发射和发射结束后的状态切换都以剩余篮球数为准。
- 修改文件：`system/zombie/catapult.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 36. 冰车和投篮车碾压只处理单株植物

- 问题描述：车辆碾压命中一格后只压扁首个植物，未按原版压扁同格全部可碾压植物；现在命中后按同一行列遍历，并按原版 `SquishAllInSquare` 语义只排除地刺和地刺王，压扁该格其他植物。
- 修改文件：`system/zombie/zombie_system.cpp`
- 修改提交号：`357d73d57db5b46fd51456bccb6d6c8978aee705`（修正 `d391f0d3191a3001e4f23e3e758debb98bfcd73d` 中该项）

## 37. 巨人投出的小鬼没有继承减速倒计时

- 问题描述：被减速巨人投出小鬼时没有复制巨人的减速倒计时，导致小鬼节奏偏快；现在投出小鬼继承 `countdown.slow`。
- 修改文件：`system/zombie/gargantuar.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 38. 火豌豆溅射覆盖矩形宽度过窄

- 问题描述：火豌豆溅射复用主攻击框宽度，覆盖范围约 30，低于原版火球溅射宽度 100；现在仅在溅射覆盖判断中使用 100 宽矩形。
- 修改文件：`system/projectile/projectile_system.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 39. 火炬树桩预命中判定缺少前探距离

- 问题描述：豌豆检测即将命中火炬树桩时直接用当前攻击框，缺少原版约 `x += 40` 的前探；现在火炬预命中检测使用前移 40 的 projectile 矩形。
- 修改文件：`system/projectile/projectile_system.cpp`
- 修改提交号：`d391f0d3191a3001e4f23e3e758debb98bfcd73d`

## 40. 土豆雷对撑杆僵尸的触发矩形条件写反

- 问题描述：土豆雷把原版本应只作用于撑杆僵尸的 `x += 40`、`width -= 40` 特殊触发框误用于所有非撑杆僵尸，导致普通僵尸触发范围偏窄且撑杆僵尸缺失经典特殊触发框；现在只在目标是撑杆僵尸时缩窄攻击矩形。
- 修改文件：`system/plant/plant_base.cpp`
- 修改提交号：`7f19d7ec8f17dfc8248dc455b3d7e7815c591d8e`

## 41. 蹦极目标选择把应跳过的格子降权为 1

- 问题描述：第 15 项修复后仍残留目标过滤遗漏，墓碑吞噬者、玉米加农炮和受保护阳光植物所在格子没有按原版直接跳过，而是作为权重 `1` 的候选格；现在目标选择阶段直接跳过这些格子，并在抓取阶段同步排除墓碑吞噬者。
- 修改文件：`system/zombie/bungee.cpp`
- 修改提交号：`7f19d7ec8f17dfc8248dc455b3d7e7815c591d8e`

## 42. 催眠僵尸不会攻击无头但未死亡的敌方僵尸

- 问题描述：催眠僵尸寻找敌方僵尸目标时使用 `!is_not_dying` 排除了所有无头核心状态，而原版只排除真正死亡或濒死阶段；现在只排除 `is_dead` 和死亡状态，允许攻击无头但仍活动的敌方僵尸。
- 修改文件：`system/zombie/zombie_system.cpp`
- 修改提交号：`7f19d7ec8f17dfc8248dc455b3d7e7815c591d8e`

## 43. 泳圈僵尸会消耗泳池/雾夜自然出怪类型容量

- 问题描述：`ducky_tube` 可在泳池/雾夜被自然出怪允许表选中并消耗类型容量，但原版 `InitZombieWavesSurvival` 会跳过泳圈僵尸；现在从自然出怪候选和场景可自然出怪判定中排除 `ducky_tube`。
- 修改文件：`system/spawn.cpp`
- 修改提交号：`7f19d7ec8f17dfc8248dc455b3d7e7815c591d8e`

## 44. W9/W19 红字归零帧刷新晚 1 帧

- 问题描述：红字倒计时旧值为 `1` 时，`spawn::update()` 先决定跳过普通刷怪更新，再把 `hugewave_fade` 递减到 `0` 并设置下一波倒计时为 `1`，导致归零帧没有像原版一样继续普通倒计时并刷新下一波。现在先递减红字倒计时，再用递减后的 `hugewave_fade` 判断是否跳过普通刷怪更新，保留递减后 `725` 的特殊放行，并让归零帧继续刷新。
- 修改文件：`system/spawn.cpp`
- 修改提交号：`8d495a0640680f8dc81b8d4ef4d2425a7ad862bb`

## 45. W20 清场后不会提前进入关末白字

- 问题描述：W20 结束阶段旧逻辑只在 5500 帧 timeout 归零后启动 500 帧关末倒计时，缺少原版 `TrySpawnLevelAward()` / `FadeOutLevel()` 在场上没有敌方僵尸时立即进入关末白字的路径，导致 PE24 关卡帧数比 1051 注入运行多约 5000 帧。现在按有头核心、非死亡/濒死、非魅惑的敌方僵尸扫描补齐清场触发，并让清场和 timeout 共用 500 帧倒计时启动逻辑。
- 修改文件：`system/spawn.cpp`、`system/spawn.h`、`tests/petest.cpp`
- 修改提交号：`2955e150b747d738beb08d3013ebecccb4289e45`

## 46. 夜间新增墓碑上限边界偏一

- 问题描述：夜间第 20 波运行中新增墓碑时，旧逻辑只在遍历到第 13 个墓碑后才提前返回，导致已有 12 个墓碑时仍可能生成第 13 个。现在按原版 `aNumGraves < 12` 语义，在已有墓碑数达到 12 时跳过新增，同时保留已有 11 个墓碑可补到 12 的行为。
- 修改文件：`system/spawn.cpp`、`tests/petest.cpp`
- 修改提交号：`4cb0dc29a8fc4f33380155a93ce9b76908aafc62`

## 47. 屋顶斜坡 projectile impact 跳过火豌豆无目标溅射

- 问题描述：屋顶/月夜直线 projectile 撞斜坡时，旧 `roof_set_disappear()` 直接销毁 projectile；原版 `Projectile::CheckForHighGround()` 在斜坡阈值命中时调用 `DoImpact(nullptr)`，其中 fireball/fire pea 会继续执行 `DoSplashDamage(nullptr)`。这导致 PE 漏掉火豌豆撞屋顶斜坡时对周围僵尸的无主目标溅射。现在斜坡阈值命中统一走 `attack_target_zombie(proj, nullptr)`，保留非溅射 projectile 的销毁语义，并补充回归测试。
- 反编译 / objdump / wiki 依据：`PlantsVsZombies-decompilation/Lawn/Projectile.cpp` 的 `Projectile::CheckForHighGround()` 阈值分支调用 `DoImpact(nullptr)`；PvZ 1.0.0.1051 `Projectile::CheckForHighGround` `0x46d0d0` 在 `0x46d10e-0x46d118`、`0x46d12f-0x46d139`、`0x46d150-0x46d15a` 跳转 `DoImpact(nullptr)`，`Projectile::DoImpact` `0x46e000` 在 `0x46e03c-0x46e05e` 对 `0x3/0x5/0x6` 溅射 projectile 调用 splash 路径；PVZ Wiki `攻略:斜坡与上界之风` 和 `技术:生存无尽入门` 均记录屋顶斜坡会阻挡射手类子弹。
- 修改文件：`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`4cb0dc29a8fc4f33380155a93ce9b76908aafc62`

## 48. 土豆雷爆炸圆心横坐标偏右 20px

- 问题描述：`damage::activate_plant()` 对土豆雷复用通用植物中心 `p.x + p.attack_box.width / 2`，实际爆心为 `x+40,y+40`；原版 `Plant::DoSpecial()` 和 PVZ Wiki 精确数据均指向土豆雷命中圆心 `x+20,y+40`、半径 `60`。现在土豆雷专用爆心改为 `p.x + 20,p.y + 40`，避免秒杀圆和同爆心格梯子清除判定整体右偏 20px。
- 修改文件：`system/damage.cpp`、`tests/petest.cpp`
- 修改提交号：`36bec6fd2e263096d04a7a12bf37d1340933de1f`

## 49. 催眠 / 右行潜水僵尸 walking 状态错误落入入水更新

- 问题描述：`zombie_snorkel::update()` 的 `snorkel_walking` 分支只在非右行路径返回，导致催眠潜水僵尸等 `is_walk_right()` 为真的 walking 状态直接 fallthrough 到 `snorkel_jump_in_the_pool`，可能错误更新 `dy`、切到 `snorkel_swim` 并设置 `is_in_water`。现在 walking 分支无论方向都会返回，右行 / 催眠潜水不会执行入水状态更新。
- 修改文件：`system/zombie/snorkel.cpp`、`tests/petest.cpp`
- 修改提交号：`36bec6fd2e263096d04a7a12bf37d1340933de1f`

## 50. 蹦极带走后的植物仍留在 `plant_map` 中参与种植判定

- 问题描述：PE 在蹦极抓取动画开始时把目标植物设为 `invisible_and_edible`，在真正上升时改为 `invisible_and_not_edible`，但旧逻辑仍把该植物保留在 `plant_map` 中，导致上升携带阶段继续参与普通占位、底座、升级、咖啡豆和坚果 / 南瓜急救修补判定。现在进入携带上升阶段时只解除目标植物的 `plant_map` 占用，不提前销毁 Plant 对象，并让 `scene` 拷贝重建 `plant_map` 时跳过 `invisible_and_not_edible` 植物。
- 反编译 / objdump / wiki 依据：原版 `Zombie::BungeeLiftTarget()` 把目标植物设为 `RISING_WITH_BUNGEE`；`Plant::NotOnGround()` 对该状态返回 true；`Board::GetPlantsOnLawn()` 在 `0x40d347` 调 `Plant::NotOnGround`，`0x40d34c-0x40d34e` 为 true 时跳过该植物；`Board::CanPlantAt` `0x40e020` 依赖 `GetPlantsOnLawn()` 分类结果。PVZ Wiki 记录蹦极落地停留后抓取并离开，叶子保护伞可保护 3x3 范围内植物。
- 修改文件：`object/scene.cpp`、`object/scene.h`、`system/plant/plant_factory.cpp`、`system/zombie/bungee.cpp`、`tests/petest.cpp`
- 修改提交号：`f11dd5d3377229f514e62856537a98d58e10f2c0`

## 51. `obj_list::get_index()` 误返回完整对象 ID

- 问题描述：`obj_list::get_index()` 在引入 DataArray generation key 后返回了完整 `key << 16 | index` ID，不再是 AvZ / 原版 DataArray 语义中的槽位 index；同时若调用方把高位 ID 临时存入 `int` 字段，旧 `get(int)` 会因负数拒绝解析。现在 `get_index()` 恢复为返回低 16 位槽位 index，稳定对象句柄统一使用 `get_id()`，`get(int)` 接受非零 bit pattern 以支持高位 ID 的旧字段存储方式。
- 反编译 / objdump / AvZ 依据：AvZ 对象 `Index()` / `Rank()` / `Id()` 分别对应完整 ID 的低 16 位、高 16 位和 32 位整体；原版 `DataArrayTryToGet()` 以 `id & 0xffff` 取槽位并比较 full ID。PvZ 1.0.0.1051 `Board::ZombieTryToGet` `0x41c7f0` 中 `movzx eax,cx` 取低 16 位作为槽位，随后比较 `[slot+0x158]` 与传入 ID。
- 修改文件：`object/obj_list.h`、`object/common.cpp`、`object/plant.cpp`、`object/projectile.cpp`、`object/zombie.cpp`、`system/plant/plant_system.cpp`、`system/plant/squash.cpp`、`system/plant/tangle_kelp.cpp`、`system/util.cpp`、`system/zombie/bungee.cpp`、`system/zombie/dancing.cpp`、`system/zombie/zombie_factory.cpp`、`system/zombie/zombie_system.cpp`、`tests/petest.cpp`
- 修改提交号：`55bb0ea9b426f8c91f59ded22e5c8f4b4f0cf60e`

## 52. 自然出怪每波当前计数未重置

- 问题描述：`spawn::gen_spawn_list()` 只在 20 波生成前清零 `curr_spawn_count`，导致冰车、白眼和旗帜波红眼的每波上限被前面波次污染。现在在每波开始时清零当前计数，同时保留 `total_spawn_count` 的全局累计语义。
- 反编译 / objdump 依据：原版 `Board.cpp` 每波调用 `ZombiePickerInitForWave()`；PvZ 1.0.0.1051 `ZombiePickerInitForWave` `0x4090f0-0x409163` 清空当前 picker 计数，`Board::PickZombieType` `0x40d8a0` 分别读取每波计数和全波计数。
- 修改文件：`system/spawn.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 53. 海豚僵尸离开水池后未切无海豚行走相位

- 问题描述：`dolphin_walk_in_pool` 到达池边时只设置 `leaving_pool` 动作和动画，没有把状态改为 `dolphin_walk_without_dolphin`。现在出池分支写入无海豚行走相位并重算速度/动画。
- 反编译 / objdump 依据：原版 `Zombie::UpdateZombieDolphinRider()` 设置 `PHASE_DOLPHIN_WALKING_WITHOUT_DOLPHIN` 后调用 `PickRandomSpeed()`；二进制 `0x5266e8` 写入 `0x38`，`0x52670a` 调速度选择。
- 修改文件：`system/zombie/dolphin_rider.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 54. 玉米投手黄油发射后状态残留

- 问题描述：玉米投手随机到黄油后，发射阶段没有清除 `kernelpult_launch_butter`，导致后续持续发黄油。现在黄油发射帧在发射前恢复 `idle`。
- 反编译 / objdump 依据：原版 `Plant::UpdateShooting()` 在黄油发射分支写回 `STATE_NOTREADY`；二进制 `0x464c10` 将 plant state 写为 `0` 后调用 `Plant::Fire()`。
- 修改文件：`system/plant/plant_system.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 55. 模仿者变身倒计时写入错误字段

- 问题描述：模仿者初始化把 200cs 倒计时写入 `countdown.effect`，更新却读取 `countdown.status`，导致种下后过早进入变身动画。现在初始化写入 `countdown.status`。
- 反编译 / objdump / wiki 依据：原版 `PlantInitialize` `0x45e2d6` 写 `[plant+0x54] = 0xc8`，`UpdateImitater` `0x466cad` 读取同一字段；PVZ Wiki 记录模仿者完整变身约 3.2 秒。
- 修改文件：`system/plant/imitater.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 56. 大喷菇升级忧郁菇丢失 pending wake 倒计时

- 问题描述：大喷菇升级忧郁菇时只在原植物已醒时恢复唤醒倒计时，睡眠但已被咖啡豆唤醒中的倒计时会丢失。现在无论是否已醒，都把保存的 `countdown.awake` 转移给新忧郁菇；已醒时仍显式清睡眠。
- 反编译 / objdump 依据：原版 `Board::MouseDownWithPlant()` 忧郁菇分支保存 `aWakeUpCounter`；二进制 `0x410aa1-0x410ab5` 对新植物执行醒着处理或写回 saved wake counter。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 57. 磁力菇会吸非 running 小丑且圆心偏上

- 问题描述：磁力菇候选阶段允许任意小丑，可能对已开盒小丑空耗冷却；吸附圆心也用 `p.x,p.y`，比原版 `mX,mY+20` 偏上。现在只接受 `jackbox_walking` 小丑，并用 `p.y + 20` 做圆心。
- 反编译 / objdump / wiki 依据：原版 `Plant::UpdateMagnetShroom()` 只接受 `PHASE_JACK_IN_THE_BOX_RUNNING`，二进制 `0x4620dd` 比较 `0x0f`，`0x46210e-0x462112` 使用 `mY + 0x14`；PVZ Wiki 记录磁力菇半径 270/320。
- 修改文件：`system/plant/mushroom_family.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 58. 杨桃跨行索敌角度使用预测右边界

- 问题描述：`zombie_base::predict_after()` 已返回预测中心，杨桃又加半宽，导致跨行角度按右边界计算。现在角度直接使用预测中心。
- 反编译 / objdump 依据：原版 `Plant::FindStarFruitTarget()` 先由 `ZombieTargetLeadX(t) - width/2` 得到左界，再加半宽回中心；二进制 `0x45f5c5-0x45f605` 对应中心角度计算。
- 修改文件：`system/plant/starfruit.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 59. 西瓜 / 冰瓜高密度溅射上限整数公式错误

- 问题描述：高密度溅射超过总上限时，PE 直接用 `7 * original / n`，瓜类变成 `560/n`。现在按原版整数顺序使用 `original * max / (splash_amount * 3)`，瓜类对应 `44800/(78n)`。
- 反编译 / objdump / wiki 依据：原版 `Projectile::DoSplashDamage()` 在 `0x46d418-0x46d423` 执行 `original * max / (splash_amount * 3)`；PVZ Wiki 精确数据记录瓜类高密溅射 `574/n`。
- 修改文件：`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 60. 蹦极拉起的火炬树桩仍阻挡豌豆

- 问题描述：`projectile_system::is_in_torchwood()` 遍历 `scene.plants` 时没有排除 `invisible_and_not_edible`，蹦极真正拉起火炬后仍可能形成幽灵阻挡。现在火炬预命中排除 NotOnGround 等价状态。
- 反编译 / objdump 依据：原版 `Projectile::PeaAboutToHitTorchwood()` 要求 `!Plant::NotOnGround()`；`Plant::NotOnGround()` 对 `RISING_WITH_BUNGEE` 返回 true；二进制 `0x46cc8a-0x46cca3` 含对应排除检查。
- 修改文件：`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 61. `scene` 拷贝丢失玉米加农炮右半格占位

- 问题描述：`scene::scene(const scene&)` 重建 `plant_map` 时只登记玉米加农炮左格，复制后的右半格被视为空。现在拷贝构造按正常创建语义同时登记左右两格。
- 反编译 / PE 内部依据：原版 `Board::GetPlantsOnLawn()` / `CanPlantAt()` 对玉米炮双格占位；PE 正常 `plant_factory::create()` 和 `scene::unregister_plant_from_map()` 已维护双格语义。
- 修改文件：`object/scene.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 62. `world::check_build()` 咖啡豆读错层

- 问题描述：`check_build()` 检查咖啡豆时读取 `base` 层，导致有底座无咖啡假阳性、无底座有咖啡假阴性。现在读取并校验 `coffee_bean` 层。
- PE 内部依据：`plant_factory::create()` 把咖啡豆写入 `plant_map[row][col].coffee_bean`，action mask 铲除优先级也读取该层。
- 修改文件：`world.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 63. `scene::to_json()` 把 `endgame` 序列化成红字倒计时

- 问题描述：JSON 输出 `endgame` key 时写的是 `spawn.countdown.hugewave_fade`。现在改为输出 `spawn.countdown.endgame`。
- PE 内部依据：`scene.h` 中 `hugewave_fade` 与 `endgame` 是独立字段，W20 清场和 timeout 测试依赖 `endgame` 倒计时。
- 修改文件：`object/scene.cpp`、`tests/petest.cpp`
- 修改提交号：`6113e0aaf36f17670c30bcb21e6abc213404e50d`

## 64. 泳池 / 雾夜舞王边界行补召会被非法上下伴舞槽提前挡住

- 问题描述：`zombie_dancing::need_summon()` 遇到首个缺失伴舞槽时直接按该槽行号返回，导致泳池 / 雾夜边界陆路的非法上下槽挡住后续合法左右补召。现在非法上下伴舞槽只跳过，继续检查其他缺失槽。
- 反编译 / objdump 依据：原版 `Zombie::NeedsMoreBackupDancers()` 在 `0x528aa0-0x528aa2` 对非法上下伴舞行跳到循环继续点 `0x528ae4`，`0x528af6` 才是发现合法缺位时返回 true。
- 修改文件：`system/zombie/dancing.cpp`、`tests/petest.cpp`
- 修改提交号：`2b482b90c5ac58d366c9ec70bc4e989863f11f19`

## 65. 红眼全轮上限未在同一波内逐次重算

- 问题描述：出怪列表生成原先每波只构建一次候选和权重，红眼全轮计数在同一波内达到上限后仍可能继续被旧候选抽中。现在每填一个出怪槽前重新构建候选，让已选数量立即参与后续过滤。
- 反编译 / objdump / wiki 依据：原版 `Board::PickZombieWaves()` 每个槽调用 `PickZombieType()`；二进制 `0x40a094` 循环内调用 `0x40d8a0`，`0x40d9e2-0x40da37` 在 picker 内按当前全轮 / 本波计数处理红眼 cap；PVZ Wiki 出怪机制记录红眼数量上限达到后非旗帜波不再出红眼。
- 修改文件：`system/spawn.cpp`、`tests/petest.cpp`
- 修改提交号：`2b482b90c5ac58d366c9ec70bc4e989863f11f19`

## 66. 蹦极 / 投篮 / 篮球顶层植物选择没有完整等价 `Plant::NotOnGround()`

- 问题描述：蹦极、投篮车和篮球多处直接读取 `plant_map` 顶层，已压扁、已死、倭瓜攻击中或蹦极拉起但仍残留在格子指针中的植物可能错误挡住南瓜 / 底座。现在抽出 `plant_not_on_ground()` 与 `get_top_plant_at()`，只在本项范围内统一过滤这些不应参与格子顶层查询的植物，并保留 `invisible_and_edible` 作为仍在地面查询中的状态。
- 反编译 / objdump / wiki 依据：原版 `Plant::NotOnGround()` `0x463ef0-0x463f27` 过滤倭瓜 5/6/7、`mSquished`、bungee rising、dead；`Board::GetPlantsOnLawn()` `0x40d347-0x40d34e` 在分类前调用并跳过；投篮与蹦极 / 篮球使用对应 top plant order，Wiki 投篮和栈位资料与该边界一致。
- 修改文件：`object/plant.h`、`system/util.h`、`system/util.cpp`、`system/zombie/bungee.cpp`、`system/zombie/catapult.cpp`、`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`2b482b90c5ac58d366c9ec70bc4e989863f11f19`

## 67. `total_flags` reset 配置缺少最低饱和轮数校验

- 历史问题：当时新增的 `world_reset_config::total_flags` 可传入低于高周目饱和语义的值，而 PE spawn 生成只实现 `>=126` flags 的高周目近似；该版曾让 C++ `world::reset(config)` 拒绝低于 126，并由当时的 Rust bridge 预校验。
- 本轮还原状态：`world_reset_config`、`world::reset(config)` 重载和 PE 仓库内 Rust bridge 已按 facade rollback 计划删除；确定性 setup、`total_flags` 支持域校验及 typed unsupported 策略迁到 RustVsZombies adapter/core。这里保留为历史修复记录，不表示这些接口仍属于当前 PE API，也不得据此恢复 facade。
- 反编译 / objdump / wiki 依据：二进制 `0x4256e4-0x42583b` 为 survival allowed-list 容量 / 低 flags 过滤，`0x42579f-0x4257c0` 为低 flags 下冰车 / 白眼 / 红眼过滤，`0x40d984-0x40da78` 为 flags 驱动的 cap / 权重；PVZ Wiki 记录 126f 起级别上限严格意义上不再限制。
- 修改文件：`world.h`、`world.cpp`、`crates/pe-rs/cpp/bridge.cpp`、`crates/pe-rs/src/world.rs`、`crates/pe-rs/src/lib.rs`、`tests/petest.cpp`
- 修改提交号：`2b482b90c5ac58d366c9ec70bc4e989863f11f19`

## 68. 开盒中的小丑僵尸可被冻结或被黄油定身

- 问题描述：`zombie::can_be_freezed()` 未排除 `jackbox_pop`，导致已进入 110cs 开盒窗口的小丑可被寒冰菇写入冻结倒计时，或被黄油写入 400cs 定身，从而错误暂停爆炸窗口。现在开盒小丑仍可被 slow/chill，但不会进入 freeze/butter 定身，且开盒倒计时继续推进；普通 20 点冻结伤害本来已被 `damage::take()` 的 `jackbox_pop` 早退挡掉。
- 反编译 / objdump / wiki 依据：原版 `Zombie::CanBeFrozen()` 排除 `PHASE_JACK_IN_THE_BOX_POPPING`；二进制 `0x531a4d cmp ecx,0x10; 0x531a50 je 0x531a7b` 返回 false，`HitIceTrap` 先 `ApplyChill(true)` 再检查 `CanBeFrozen()`，`ApplyButter` 也共用该门禁；PVZ Wiki 记录小丑开盒后冰蘑菇冻不住，减速不暂停开盒倒计时。
- 修改文件：`object/zombie.cpp`、`tests/petest.cpp`
- 修改提交号：`2b482b90c5ac58d366c9ec70bc4e989863f11f19`


## 69. N1：产阳 helper 重置天降阳光倒计时

- 问题描述：`sun` 构造改为无副作用，产阳植物只增加余额；首次自然阳光倒计时由显式 reset 初始化，植物产阳不再覆盖计时或额外消费 RNG。
- 依据 / 验证：原版 `Plant::UpdateProductionPlant` 不写 Board 阳光倒计时，只有 `Board::UpdateSunSpawning@0x413a70` 维护该字段。
- 修改文件：`system/sun.cpp`、`system/sun.h`、`system/plant/plant_base.cpp`、`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 70. N2：首次天降阳光缺少 700 端点

- 问题描述：首次倒计时改为 `425 + randint(276)`，支持集恢复为 425..700；后续掉落继续使用原版宽度 275。
- 依据 / 验证：Board 初始化 objdump 使用 `0x114 = 276` 后加 `0x1a9 = 425`。
- 修改文件：`system/sun.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 71. N3：植物初始动画 FPS 重复抽样且下界错误

- 问题描述：植物初始化只抽样一次 `randfloat(10, 15)`，不再先抽后清零再按 5..15 重抽；墓碑吞噬者落地时序恢复。
- 依据 / 验证：`PlantInitialize@0x45db60` 的机器码形成 10..15，墓碑分支不再覆盖 FPS。
- 修改文件：`system/plant/plant_base.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 72. N4：被压扁植物残留在 plant_map

- 问题描述：`set_smashed()` 在进入 500cs 消失等待时立即注销植物格位，补种、升级和顶层查询不再命中已压扁对象。
- 依据 / 验证：原版 `Plant::NotOnGround()` 排除 `mSquished`；PE 格位生命周期现在与该语义一致。
- 修改文件：`system/damage.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 73. N5：玉米炮右格接受已离地玉米投手

- 问题描述：玉米炮升级的右半格验证改读 `plant_map` 的在地 content，不再遍历对象池命中被蹦极拉起的玉米投手。
- 依据 / 验证：原版双格种植判定依赖 Board 格位中的在地植物。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 74. N6：灰烬斩杀蹦极未先释放抓取植物

- 问题描述：灰烬/瞬杀路径在写死亡状态前调用 bungee drop 逻辑，恢复正在抓取植物的 edible 状态并清目标 ID。
- 依据 / 验证：原版 bungee 在死亡/灰烬处理前执行目标释放；1051 相位 7 路径与反编译一致。
- 修改文件：`system/damage.cpp`、`system/zombie/bungee.cpp`、`system/zombie/zombie.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 75. N7：大嘴花状态切换沿用旧动画进度

- 问题描述：bite、chew、swallow 的状态转换统一通过 `set_reanim()`，同时复位 progress、prev_progress 和循环计数。
- 依据 / 验证：原版每次切换对应 reanimation 都从新动画起点开始。
- 修改文件：`system/plant/chomper.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 76. N8：倭瓜二次索敌为空时卡住

- 问题描述：倭瓜的起跳状态推进与 map 注销移出可选目标分支；锁定目标消失时仍按既定目标坐标完成起跳流程。
- 依据 / 验证：原版倭瓜 phase 推进不以二次 FindTarget 成功作为必要条件。
- 修改文件：`system/plant/squash.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 77. N9：磁力菇排序使用僵尸锚点

- 问题描述：磁力菇候选距离改用僵尸 hit rect 左上角 x/y，避免模型锚点偏移改变吸取优先级。
- 依据 / 验证：原版 Magnet 目标排序读取 ZombieRect；1051 objdump 与反编译均以碰撞矩形计算。
- 修改文件：`system/plant/mushroom_family.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 78. N10：报纸/梯子啃食事件相位过早

- 问题描述：报纸和梯子僵尸的魅惑菇/大蒜特殊事件门槛改为 0.42，普通僵尸仍使用 0.14。
- 依据 / 验证：Wiki 精确数据与原版啃食事件分支均给出报纸、梯子 0.42。
- 修改文件：`system/zombie/zombie_system.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 79. N11：跳跳断头未执行 PogoBreak

- 问题描述：普通断头伤害现在立即移除起蹦器并切步行；带原版 `DAMAGE_DOESNT_LEAVE_BODY` 位（`0x10`；PE 遗留枚举名为 `disable_ballon_pop`）时，在设置无头核心状态并清除黄油后提前返回，不执行 `PogoBreak`，也不重置进食、相位、主动画或消耗相关 RNG。若原先有黄油，仅恢复动画 FPS。
- 依据 / 验证：`DropHead@0x529a30` 在 `0x529af6` 检查 `DAMAGE_DOESNT_LEAVE_BODY` 的 `0x10` 位，并在 `0x529bdf` 的 `PogoBreak@0x525350` 调用前返回；黄油清除位于早退之前。
- 修改文件：`system/damage.cpp`、`system/zombie/pogo.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 80. N12：海豚死亡动画相位门控错误

- 问题描述：只有 `dolphin_walk_in_pool (0x37)` 进入普通死亡动画，其余海豚相位立即销毁。
- 依据 / 验证：`PlayDeathAnim@0x533240` 在 `0x53329a` 比较 0x37，不等时跳立即死亡。
- 修改文件：`system/damage.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 81. N13：雪人逃跑速度仍为 0.4

- 问题描述：`yeti_escape` 的 dx 恢复为 0.8，普通雪人仍为 0.4。
- 依据 / 验证：原版 PickRandomSpeed/雪人逃跑分支与 Wiki 速度资料均区分 0.4 和 0.8。
- 修改文件：`system/reanim.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 82. N14：obj_list 每帧重建 free-list

- 问题描述：对象池新增持久 `max_used_end`，保留 `active_end` 作为迭代优化；cleanup 单次升序扫描并把新释放槽压入持久 LIFO 链，跨 cleanup 仍按 2→0 复用。
- 依据 / 验证：1051 DataArray `ProcessDeleteQueue@0x41bad0` 递减 mSize、压 free-list，但不收缩 mMaxUsedCount。
- 修改文件：`object/obj_list.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 83. N15：显式僵尸创建先随机选行

- 问题描述：显式行创建不再调用 PickRow，水池珊瑚也直接按目标行创建。显式 bungee 仍先保留原版 PickBungeeTarget 的 RNG/无候选死亡副作用，再覆写请求 row/col/x/y；普通显式 x 使用 GridToPixelX-30。
- 依据 / 验证：`AddZombieInRow@0x40ddc0` 不选随机行；`IZombiePlaceZombie@0x42a0f0` 初始化后写 target col 和请求 row。
- 修改文件：`system/zombie/zombie_factory.cpp`、`system/zombie/zombie_factory.h`、`system/zombie/bungee.cpp`、`system/zombie/zombie.h`、`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 84. N16：模仿者目标合法域过宽

- 问题描述：公共 C++/Rust bridge 与 plant factory 统一只允许模仿 0..39 绿卡，拒绝升级卡、none、自身嵌套及非模仿者携带 target。
- 依据 / 验证：ImitaterDialog/SeedHitTest@0x482d30 以 `< 0x28` 限定绿卡。
- 修改文件：`object/plant.h`、`system/plant/plant_factory.cpp`、`world.cpp`、`crates/pe-rs/cpp/bridge.cpp`、`crates/pe-rs/src/lib.rs`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 85. N17：world copy 重置中局 spawn/sun/RNG

- 问题描述：`sun`/`spawn` 构造改为 deferred 无副作用，copy constructor 只绑定 subsystem，不再重抽阳光或重置波表。
- 依据 / 验证：这是 PE 状态复制合同；复制后 RNG、wave、countdown、spawn_list 和阳光均由回归锁定。
- 修改文件：`system/sun.cpp`、`system/sun.h`、`system/spawn.cpp`、`system/spawn.h`、`world.cpp`、`world.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 86. N18：scene copy 把离地倭瓜登记回 map

- 问题描述：复制重建 plant_map 时跳过被拉起植物、攻击中的倭瓜和 smashed 植物，并保持玉米炮双格指针指向副本对象。
- 依据 / 验证：原版 `Plant::NotOnGround@0x463ef0` 对倭瓜攻击相位返回 true。
- 修改文件：`object/scene.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 87. N19：批量 update/observation 输入形状不安全

- 问题描述：批量入口先验证帧数、world/action/mask 行数和宽度、空指针及重复 world；校验完成后才改输出。worker 数有 1 的下限，空批次安全返回。
- 依据 / 验证：PE API 自身安全合同；重复 world 会让 legacy worker 并发写同一实例，现已显式拒绝。
- 修改文件：`world.cpp`、`learning/observation_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 88. N20：植物 ID 修改器接受无效/离地对象

- 问题描述：统一 live predicate 拒绝 foreign、dead、smashed、bungee lifted 和 squash_crushed；remove、HP、awake/grown、effect、炮 ready/fire/recover 共用该门禁。
- 依据 / 验证：1051/RSVZ live resolver 均把 off-board 与 squash done 排除在通用修改器之外。
- 修改文件：`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 89. N21：pybind vector 越界和悬空引用

- 问题描述：三个序列实现 Python 负索引与 IndexError；Batch 行按值返回，删除会因 assign/realloc 失效的显式 STL iterator，避免旧 row/iterator UAF。
- 依据 / 验证：Python 序列/API 内存安全合同；独立 smoke 覆盖正负越界和容器重分配反例。
- 修改文件：`pybind.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 90. N22：world(scene_type) eager/deferred 合同冲突

- 问题描述：裁决为 deferred：构造和复制不生成波表/自然阳光，调用方必须显式 reset；main、README、AGENTS 与测试已统一。
- 依据 / 验证：符合项目 AGENTS 的重量级 RNG 初始化显式触发原则。
- 修改文件：`AGENTS.md`、`README.md`、`main.cpp`、`system/spawn.cpp`、`system/spawn.h`、`system/sun.cpp`、`system/sun.h`、`world.cpp`、`world.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 91. N23：非 straight projectile 也进入火炬提前避碰

- 问题描述：火炬 lookahead 增加 `MOTION_STRAIGHT` 门槛，反向/跨行等运动不再被错误挡弹。
- 依据 / 验证：`PeaAboutToHitTorchwood@0x46cc30` 只服务直线豌豆路径。
- 修改文件：`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 92. N24：ObservationFactory 32 位容量乘加溢出

- 问题描述：构造和批量输出尺寸改用受检 size_t 乘加，并先验证各对象上限与 batch 形状，溢出/超容量抛 invalid_argument。
- 依据 / 验证：PE learning API 内存安全合同；极大参数回归覆盖旧小分配远写风险。
- 修改文件：`learning/observation_factory.cpp`、`learning/observation_factory.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 93. N25：plant sentinel 在费用表前未校验

- 问题描述：`can_plant/create/get_cost` 在任何数组索引前验证植物类型和模仿目标；非法 create/get_cost 抛异常，查询返回 false。
- 依据 / 验证：原版卡片域与 PE 固定表边界共同要求拒绝 none/越界枚举。
- 修改文件：`object/plant.h`、`system/plant/plant_factory.cpp`、`world.cpp`、`crates/pe-rs/cpp/bridge.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 94. N26：关末白字前仍更新最终波伏兵

- 问题描述：对象/子弹/卡片更新后立即尝试 clear-field endgame，再在 sun/spawn 前建立门控；白字期间伏兵倒计时冻结且不会生成新敌人。
- 依据 / 验证：1051 `Board::UpdateZombieSpawning@0x413d00` 的关末状态在后续生产/出怪前阻断对应逻辑；真正的 `Board::UpdateGame` 入口为 `0x415920`。
- 修改文件：`world.cpp`、`system/spawn.cpp`、`system/spawn.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 95. N27：关末白字仍推进自然阳光

- 问题描述：`sun::update` 在 endgame 倒计时非零时直接返回。清场触发帧不推进阳光；spawn timeout 触发发生在 sun 之后，因此该帧仍合法推进一次，后续冻结。
- 依据 / 验证：回归分别锁定 clear-trigger 与 timeout-trigger 的原版帧序差异。
- 修改文件：`system/sun.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 96. D1：plant JSON cannon 坐标写错

- 问题描述：`plant::to_json()` 的 cannon 字段改写 `cannon.x/y`，不再重复植物自身 x/y。
- 依据 / 验证：PE 调试序列化字段自洽性。
- 修改文件：`object/plant.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 97. D2：obj_list::size 与迭代数量不一致

- 问题描述：`size()` 现在统计 iterator-visible live 对象；内部 n_actives 继续保留 DataArray mSize 语义。Python __len__ 直接复用 size，observation 保留单遍迭代。
- 依据 / 验证：原版 Board iterator 会额外跳过 mDead；测试在 cleanup 前锁定 size==distance。
- 修改文件：`object/obj_list.h`、`pybind.cpp`、`system/spawn.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 98. D3：check_build 缺少格子边界检查

- 问题描述：外部 check_list 的 row/col 在访问 plant_map 前验证，非法输入安全返回 false。
- 依据 / 验证：PE 公共 API 边界合同。
- 修改文件：`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 99. D4：update_all frames=0 无符号下溢

- 问题描述：`frames == 0` 在任何输出或 world 推进前抛 invalid_argument。
- 依据 / 验证：PE 批量 API 边界合同；回归验证 world/输出保持未改。
- 修改文件：`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 100. P1：observation 重复首对象且冰道槽位错误

- 问题描述：对象填充循环正确递增 iterator 并受 end/max 双重约束；冰道 x/countdown 分别写两个 float，除法改为浮点。
- 依据 / 验证：PE observation 布局合同。
- 修改文件：`learning/observation_factory.cpp`、`learning/observation_factory.h`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 101. P2：空卡槽索引 CD_TABLE[-1]

- 问题描述：卡槽 target_type 先做范围检查，none/非法目标的 cooldown 特征精确写 0。
- 依据 / 验证：PE observation 内存安全合同；十个空槽逐项断言 0。
- 修改文件：`learning/observation_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 102. P3：批量 helper 对 0 worker/空输入不安全

- 问题描述：worker 数使用 `max(1, hardware_concurrency())` 并受 batch 大小限制；空 world/mask 批次返回空输出，非空形状严格校验。
- 依据 / 验证：C++ 标准允许 hardware_concurrency 返回 0；PE legacy API 现有定义行为。
- 修改文件：`world.cpp`、`learning/observation_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 103. P4：PlantCountdown pybind 链缺少分号

- 问题描述：补齐绑定声明终止符，pybind 翻译单元可完整编译。
- 依据 / 验证：Python 模块 Release 全量构建验证。
- 修改文件：`pybind.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 104. P5：CMake 模块名与 PyInit 名不一致

- 问题描述：CMake target 使用 MODULE 并把输出名设为 `pvzemu`；同时改用 Python 发现的 pybind11 3.x config，支持 CMake 4/Python 3.13 和中文路径 UTF-8。
- 依据 / 验证：产物 `pvzemu.cp313-win_amd64.pyd` 已构建并成功 import。
- 修改文件：`CMakeLists.txt`、`README.md`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 105. P6：setup.py 缺少 rapidjson include

- 问题描述：setup.py include_dirs 加入 `./lib`，pip/MSVC 路径可找到 bundled rapidjson。
- 依据 / 验证：强制全量 setup.py 构建通过。
- 修改文件：`setup.py`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 106. P7：setup.py MSVC 参数不支持 C++17

- 问题描述：MSVC 参数加入 `/std:c++17` 并删除无效裸 `/arch`；pybind 最低依赖同步到 3.0。
- 依据 / 验证：Python 3.13 MSVC 编译行与导入 smoke 已核验。
- 修改文件：`setup.py`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 107. 独立核验新增：Python observation 暴露 raw float* 写越界

- 问题描述：删除 Python 侧 `ObservationFactory.create(..., float*)` overload，只保留拥有明确容量的 FloatVector/批量 vector 入口；传入 0.0 现在 TypeError，不再进程崩溃。
- 依据 / 验证：独立 agent 在旧绑定稳定复现 0xC0000005，修复后反例被拒绝。
- 修改文件：`pybind.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 108. 独立核验新增：scene 引用型 Python wrapper 可悬空

- 问题描述：Sun、Spawn、各 system/factory 等 11 个 `scene&` 构造器添加 `keep_alive<1,2>`，wrapper 生命周期内保持 Scene 存活。
- 依据 / 验证：weakref/GC smoke 验证 Scene 不会在 wrapper 前释放。
- 修改文件：`pybind.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 109. 独立核验新增：README 错误宣称全部线程安全

- 问题描述：公开合同改为单个 world 必须固定在一个线程，不同线程可分别拥有独立 world；update_all 也拒绝重复实例指针。
- 依据 / 验证：与 PE 单 world 单线程状态模型及 Rust runner 每 worker 独占 world 的合同一致。
- 修改文件：`README.md`、`world.cpp`、`tests/petest.cpp`
- 修改提交号：`3b96f225a8d171150cf7cf225a2f988f5580a7b5`

## 110. N28：模仿者占位植物按有效类型登记 plant_map

- 问题描述：植物创建与 scene copy 重建 `plant_map` 时，模仿者现在使用 `imitater_target` 作为有效类型，将模仿花盆/睡莲、咖啡豆、南瓜分别登记到 base、coffee_bean、pumpkin 层；普通 content 与变身后的真实植物不再被占位对象错误覆盖或清空。
- 依据 / 验证：`test_imitater_map_copy_and_cooldown_remap` 覆盖模仿睡莲、模仿南瓜、scene copy 指针重建与 320cs 变身后的真实 content；实现与 1051 `Board::GetPlantsOnLawn` 的有效类型分层一致。
- 修改文件：`object/scene.cpp`、`system/plant/plant_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 111. N29：睡眠、压扁和离地植物仍推进特效倒计时

- 问题描述：`plant_system::update()` 统一计算 ability gate；睡眠、压扁、死亡或非正常 edible 的植物既不装填攻击，也不递减 `countdown.effect` 或触发特殊效果，恢复可更新状态后才继续推进。
- 依据 / 验证：`test_plant_effect_and_seed_modifiers` 同时覆盖普通递减、instant-special 立即生效和睡眠植物冻结倒计时；门禁位置对应 1051 `Plant::UpdateAbilities` 在特效倒计时之前的整段早退。
- 修改文件：`system/plant/plant_system.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 112. N30：EasyPlantingCheat 与空间限制、阳光和冷却语义混合

- 问题描述：easy cheat 不再与 `ignore_planting_restrictions` 合并成通用空间规则短路；水路、墓碑、容器和叠层等规则仍执行，只在原版允许的升级底材、空位玉米炮和香蒲分支放宽。easy 同时让 seed 可取并跳过阳光 admission/扣费，已有冷却不再阻止使用。
- 依据 / 验证：`test_easy_planting_and_restriction_rules` 覆盖裸水路仍拒绝普通植物、独立 restriction-ignore 放开空间规则、升级卡/玉米炮 easy 分支、零阳光与非零冷却可用且不扣费。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 113. N31：删除错误的 PE world 炮恢复时间 helper

- 问题描述：删除会漏算 firing/charging 动画的 `world::cob_recover_time()` 及其 PE world 高层 facade，不在模拟器内继续维护近似恢复公式；炮恢复时间由 RSVZ 从 plant 状态、倒计时和 reanim 进度原子统一组合。
- 依据 / 验证：还原 hunk 审计确认 PE 的 world 声明、实现和旧公式测试均已清除；`rsvz-game` 炮 timing/manager 回归从后端原子事实计算恢复时间，PE 核心不再提供重复策略实现。
- 修改文件：`world.h`、`world.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 114. N32：更换模仿者目标时错误清零未完成冷却

- 问题描述：生存重新选卡按 packet type 保存旧卡冷却；模仿目标不变时保留 remaining，目标改变时先由旧目标总 CD 推回 elapsed，再按新目标总 CD 计算 remaining，不再直接归零。
- 依据 / 验证：`test_imitater_map_copy_and_cooldown_remap` 覆盖普通卡换位保留冷却，以及模仿坚果改为模仿樱桃后的 elapsed 映射；与 1051 SeedChooser 按 `SEED_IMITATER` 保存 elapsed counter 的流程一致。
- 修改文件：`world.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 115. N33：自然蹦极参与泳池水路初始抽行

- 问题描述：bungee 从 pool/fog 可进入水路的通用类型组移到陆路限定组；自然旗帜波蹦极不再更新或重置水路共享行平滑历史，最终目标行仍由蹦极目标选择决定。
- 依据 / 验证：`test_zombie_creation_and_motion_regressions` 在 pool 中创建自然蹦极后断言两条水路的 row-random 历史未被污染；合法行集合与 1051 `ZombieTypeCanGoInPool` 不含 bungee 一致。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 116. N34：海豚跨越植物时高度固定为 0

- 问题描述：`dolphin_jump` 按 `countdown.action` 的 120→0 线性区间把高度从 0 推进到 10，并对输入端点钳制；判定框不再在整段跳跃中错误保持地面高度。
- 依据 / 验证：`test_zombie_creation_and_motion_regressions` 分别断言 action 120/60/0 时高度为 0/5/10；对应 1051 `TodAnimateCurveFloat` 的线性曲线参数。
- 修改文件：`system/zombie/dolphin_rider.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 117. N35：矿工出土曲线在 40～35 区间错误外推

- 问题描述：矿工第二段出土曲线先把 action 钳制到 0..30；action 40..30 保持高度 20，负倒计时钳到终点 0，不再把 ease-in 多项式外推到定义域之外。
- 依据 / 验证：`test_zombie_creation_and_motion_regressions` 覆盖 action -5、30、40 的 0/20/20 高度；行为对应 1051 `TodCurveEvaluateClamped` 的端点钳制。
- 修改文件：`system/zombie/digger.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 118. N36：显式 spawn_wave 未传播到多数僵尸类型

- 问题描述：`create_in_row_impl()` 在所有类型初始化后统一写入请求 wave，并只在 huge-wave 归属变化且该类型使用基础出生位置时校正 x/y/int_x/int_y；显式波次不再只对伴舞和小鬼生效。
- 依据 / 验证：`test_zombie_creation_and_motion_regressions` 覆盖普通僵尸 wave 8/9 的 x+40、伴舞/小鬼/蹦极/巨人的大波次值，以及 Z09 非法 kind/row/from_wave 边界。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 119. D5：projectile JSON 错译目标范围 flags

- 问题描述：`projectile::to_json()` 现在把成员 `flags` 按 `attack_flags` 导出 ground、balloon、snorkel、dog、动画/死亡/矿工/魅惑目标范围；不再冒充命中时动态生成的 `zombie_damage_flags`，debugger TypeScript 字段同步。
- 依据 / 验证：`test_grid_atoms_and_debug_json_flags` 构造 ground+dog+hypno flags，断言 JSON 中 dog/hypno 为 true、flying_balloon 为 false。
- 修改文件：`object/projectile.cpp`、`debugger/src/objects/Projectile.ts`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 120. P9/P10：Python factory 返回对象身份和 owner 生命周期错误

- 问题描述：Plant/Zombie/Griditem factory 的 `create()` 与 `PlantFactory.plant()` 统一使用 `reference_internal`，返回真实对象池实体而非 Python 副本，并让返回 wrapper 保活 factory；factory 构造器已有的 scene keep-alive 继续把生命周期链延伸到 World。
- 依据 / 验证：Release Python smoke 覆盖返回对象与 scene iterator 的身份、修改/销毁作用于真实池对象，以及删除 World、强制 GC 和堆复用后对象仍可安全读取，不再出现副本脱节或悬空引用。
- 修改文件：`pybind.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 121. R1：随机源未区分 Battle / Level 且输出范围不同

- 问题描述：PE 原先直接使用 `std::mt19937` 的 32-bit 输出，并把战斗随机与关卡局部随机混在同一条流中。现保留标准库分布与直接合法候选抽样，只把基础随机源限制为 31-bit，并拆分 Battle/Level 两条流；跨轮只按原始 Level seed `+101` 重播种，不重建整个 world。
- 语义边界：PE 只保证炮伤、砸率、刷新和通关数量等宏观统计，不保证同 seed 与 1051 得到相同对象、波表或调用序列；因此不补视觉/未建模系统的空随机调用，也不维护生产态调用摘要。
- 依据 / 验证：1051 使用独立的 Battle 与关卡局部随机流；`test_original_rng_contract_and_pool_capacity` 冻结31-bit基础输出、双流独立和跨轮`+101`行为，不冻结允许表或随机调用数量。
- 修改文件：`system/random_stream.h`、`system/spawn.cpp`、`world.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`615440fb21fc9acf94ee9ba42978ef9d7894af6c`、`dfed054d763229a1bb0f81b5918530000c28083e`

## 122. R2：植物对象池容量只有原版的一半

- 问题描述：PE 的植物池容量为 512，而 1051 `Board::mPlants` 的固定 DataArray 容量为 1024；长程场景会比原版更早耗尽植物槽，也无法用 Witness 对齐完整的槽位、空闲链和代际状态。现将植物池容量改为 1024。
- 依据 / 验证：1051 Board 布局与原生 DataArray 容量为 1024；`test_original_rng_contract_and_pool_capacity` 冻结 PE 植物池容量，`test_obj_list_native_index_and_generation_id` 覆盖槽位复用和代际回绕。
- 修改文件：`object/scene.h`、`tests/peparitytest.cpp`、`tests/petest.cpp`
- 修改提交号：`615440fb21fc9acf94ee9ba42978ef9d7894af6c`

## 123. C1：空间种植判定与卡片资源判定接口混合

- 问题描述：新增只返回空间/规则类 `planting_reason` 的 `plant_factory::can_plant_at()`，并用 `can_pick_up()` 单独处理卡片冷却、阳光与种植前置条件；`can_plant()` 明确保留为“当前是否可立即种下”的组合判定。删除语义含混的 `world::can_place_plant()` facade 后，空间合法性查询不再被阳光余额污染。
- 依据 / 验证：`tests/petest.cpp` 覆盖 `can_plant_at()` 的场景、叠层、底座、升级植物和 easy-cheat 规则；RustVsZombies 当前的 `PlantPlacementBackend::can_plant_at()` 直接桥接该纯空间判定，并与 seed 可用性 / 阳光 capability 分离。
- 修改文件：`system/plant/plant_factory.cpp`、`system/plant/plant_factory.h`、`world.cpp`、`world.h`、`tests/petest.cpp`
- 修改提交号：`9c82a0f50eb6f894eb71666600970f3d6bf1c502`

## 124. N37：伏兵出土分支跳过公共 reanimation 推进

- 问题描述：`rising_from_ground` 原先在更新出土高度后对整个僵尸循环执行 `continue`，连公共坐标同步与 reanimation 推进也一并跳过；现改为退出状态 `switch`，只跳过 `UpdatePlaying` 后半并继续公共尾部。
- 依据 / 验证：1051 `Zombie::UpdatePlaying@0x52B340` 在 `0x52B47E` 调出土更新后只返回调用者，`Zombie::Update@0x52AE60` 随后仍执行 Animate、`mX/mY` 同步和 UpdateReanim；`test_rising_zombie_reanimation_keeps_advancing` 冻结该层级。
- 修改文件：`system/zombie/zombie_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 125. N38：咖啡豆被冰道和 raw griditem 通用门禁提前拒绝

- 问题描述：`can_plant_at()` 原先在咖啡豆专用判定前统一检查冰道、弹坑和墓碑，导致既有睡眠蘑菇处于这些状态时无法唤醒；现把完整 coffee 专支移到通用 griditem/ice 门禁之前，并保留 restriction-ignore 策略。
- 依据 / 验证：1051 `Board::CanPlantAt@0x40E020` 的 seed `0x23` 分支位于 `0x40E118-0x40E15B` 并直接返回，crater 与 `IsIceAt` 分别到 `0x40E1B9`、`0x40E1E7` 才检查；回归覆盖自然冰道及同格 raw crater+grave，普通植物仍受冰道限制。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 126. N39：香蒲可升级正在被蹦极抓取的睡莲

- 问题描述：香蒲专用 base-upgrade 分支只验证睡莲类型和空 content，漏掉 `GETTING_GRABBED_BY_BUNGEE` 门禁；现与其它升级底材一致，拒绝 `invisible_and_edible` 的正在抓取睡莲。
- 依据 / 验证：1051 `Board::CanPlantAt` 在 `0x40E388` 检查可升级后，于 `0x40E391-0x40E39C` 额外比较 under plant `+0x134 != 1` 才返回 OK；回归同时覆盖抓取态拒绝与正常睡莲允许。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 127. N40：raw `add_crater()` 的零倒计时弹坑永不回收

- 问题描述：strict/raw `add_crater()` 按原版创建 counter=0 的弹坑，但 PE 旧更新只在 counter 为正且递减到零时销毁，造成永久弹坑；现先按需递减正值，再对所有路径统一检查零值。
- 依据 / 验证：1051 `AddACrater@0x408F80` 不写新对象的零 counter；`UpdateGridItems@0x41D790-0x41D7A0` 正值才递减，却无条件在最终值为零时调用 `GridItemDie`。`test_grid_atoms_and_debug_json_flags` 覆盖 raw 零值立即回收。
- 修改文件：`system/griditem.h`、`tests/petest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 128. N41：海豚骑乘到左岸的离水边界误用浮点坐标

- 问题描述：`dolphin_ride` 用当前浮点 `x > 10` 判断继续骑乘，在 `x∈(10,11)`、`int_x==10` 时比原版晚 1cs 离水；现改读上一帧同步的整数 `int_x`。
- 依据 / 验证：1051 `UpdateZombieDolphinRider@0x5261E0` 在 `0x5263E4` 比较 `[edi+0x08]`（整数 `mX`）与 10；回归构造 `x=10.5/int_x=10` 并断言立即进入 leaving-pool。
- 修改文件：`system/zombie/dolphin_rider.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 129. N42：历史第 19 项进家边界修复换错字段且漏掉无头等号

- 问题描述：历史修复把 `int_x <` 改成了浮点 `x <=` / `x <`；现恢复原版上一帧整数 `int_x` 语义，并让有头进家和无头 `edge+70` 强杀两处都使用 `<=`。
- 依据 / 验证：1051 `CheckForBoardEdge@0x52B280` 在 `0x52B2F1`、`0x52B31E` 都比较 `[esi+0x08]`，随后用 `jg` 跳过；回归覆盖浮点已越界但 int 尚未越界不提前判负，以及无头恰等边界仍受 1800 伤害。
- 修改文件：`system/zombie/zombie_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

## 130. N43：日/夜行号缺少上界夹取使底行杨桃星漏伤

- 问题描述：day/night `get_row_by_x_and_y()` 只夹下界，底行右下杨桃星在 `y>580` 后被写成不存在的 row 5，从而跳过同行碰撞；现使用标准库把五行场景结果夹到 `0..4`。
- 依据 / 验证：1051 `PixelToGridY@0x41C550` 在 `0x41C625` 计算后复用 `0x41C5E9-0x41C5F4` 夹到 4；自然回归在第 46cs 构造 projectile/僵尸横域 overlap=0，确认原版应命中 20 且 row 保持 4。该证据不依赖 AGENTS 排除的非法炮目标。
- 修改文件：`system/util.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`94a47a5ac2f0dc617f3764b1af3ab664e01ea104`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 配置下，`petest`、`peparitytest` 均通过；修复后 fresh-context 复审 N37-N43 全部 PASS。

## 131. N44：自然刷怪绕过原版 1023 只活跃僵尸上限

- 问题描述：显式 `add_zombie_in_row()` / `place_zombie()` 已在 1023 只时拒绝，但普通波、舞王伴舞、巨人小鬼以及 pool/night/roof 伏兵仍直接调用引用型 `create()`，可合法分配第 1024 个槽，下一次再抛 `bad_alloc`。现集中提供 nullable `try_create()` / fixed-row 入口，全部自然调用处理空返回；引用型 Legacy `create()` 也不再越过门禁。屋顶成对创建第二只失败时安全回滚首个蹦极。
- 依据 / 验证：1051 `Board::AddZombieInRow@0x40DDC0` 在 `0x40DDCC-0x40DDDC` 判断 `mSize >= mMaxSize-1` 并返回 null；普通波调用 `0x412F86/0x413059` 忽略 null，舞王和巨人分别在 `0x5287DA-0x5287E9`、`0x526FBF-0x526FCE` 处理满池。Wiki《PE G式真和平》记录 1023 只上限。回归先填满 1023 只再触发自然波，断言数量不增长。
- 历史分析：`9c82a0f` 只修复了显式原子入口，未覆盖旧自然调用链；本项补齐该历史修复。
- 修改文件：`system/zombie/zombie_factory.cpp`、`system/zombie/zombie_factory.h`、`system/spawn.cpp`、`system/zombie/dancing.cpp`、`system/zombie/gargantuar.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 132. N45：败北帧对象、卡片和主计数结算顺序整体错误

- 问题描述：PE 在首只僵尸进家时提前终止整个僵尸循环，却继续完整移动/碰撞 projectile、递减卡片冷却并增加 `main_counter`。现让触发者完成公共尾部、后续僵尸仅增龄、projectile 仅增龄；卡片、主计数、阳光、刷怪和冰道均在本帧冻结。
- 依据 / 验证：1051 `ZombiesWon@0x413400` 在 `0x413762` 立即把 GameScene 改为 zombies-won；后续僵尸在 `Zombie::Update@0x52AE60` 只执行 `0x52AE6B` 增龄，projectile 在 `0x46E464` 增龄后由 `0x46E46A` 门禁返回，SeedPacket 在 `0x487258-0x48725F` 直接返回；`Board::UpdateGame@0x415920` 于 `0x415AF0` 跳过 `mMainCounter++@0x415B29`。综合回归冻结这些同帧原子。
- 历史分析：`9c82a0f` 引入 terminal/game-over 路径时只保留了部分对象更新，未复现原版切景发生在对象循环中间的层级关系。
- 修改文件：`world.cpp`、`system/zombie/zombie_system.cpp`、`system/projectile/projectile_system.cpp`、`tests/petest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 133. N46：猫尾草追踪中心使用半像素除法

- 问题描述：追踪刺把僵尸矩形和 projectile 的整数宽高除以 `2.0`，普通僵尸 115px 高等奇数尺寸会产生原版不存在的 0.5px 偏航；现恢复四处整数 `/2`，后续浮点归一化保持不变。
- 依据 / 验证：1051 `Projectile::UpdateNormalMotion@0x46D890` 在 `0x46D927-0x46D933`、`0x46D93F-0x46D949`、`0x46D953-0x46D95D` 对目标高及弹体宽高做有符号整数除二；`ZombieTargetLeadX@0x52A580` 的 `0x52A5D4-0x52A5DD` 对目标宽同样整数除二。回归构造水平对齐中心，断言 y 速度仍为零。
- 历史分析：`bfb1c2075eb95c15959ae317cfcf60cd2b3e88dd` 名为“Fix cattail float bug”，但把部分中心计算进一步改为浮点，属于方向相反的历史修复。
- 修改文件：`system/projectile/projectile_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 134. N47：三线射手射击 hold 状态永久卡在 `launch=1`

- 问题描述：首发只把 `threepeater_time_since_first_shot` 加到 1，后续每帧反复执行 `launch 1→0→1` 却不推进辅助计时，射击 hold 永不结束。现于 30cs hold 内逐帧推进到 31，下一 cs 清零计时且让 launch 保持 0。
- 依据 / 验证：1051 `Plant::UpdateShooting@0x464820` 在 `0x464889-0x46488F` 递减，`0x464A8F-0x464B31` 于 counter=1 发射并返回；counter=0 后由 `0x46498F` 检查中央头 loopCount，未完成才在 `0x464D95` 写回 1，完成则保持 0。回归分别断言发射、30cs hold 和退出三阶段。
- 历史分析：该辅助计时和错误状态机来自初始提交 `1a5d0d48`，此前没有对应记录或修复。
- 修改文件：`system/plant/plant_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 135. N48：历史无头修复错误停止啃食并重置动画/移动速度

- 问题描述：普通僵尸首次掉头时不应通用执行 `unset_is_eating()`、重设 walking reanimation 或重抽速度。现仅设置无头核心状态、清黄油并在原先有黄油时恢复 FPS；普通 Pogo 仍执行 PogoBreak，带 `DAMAGE_DOESNT_LEAVE_BODY` 位时仍跳过该特例。正在啃食的无头僵尸保持原状态，但不再造成伤害。
- 依据 / 验证：1051 `DropHead@0x529A30` 清黄油并写 `mHasHead=false`，`0x529AF6` 检查 no-body 位；普通路径没有 StopEating 或通用 StartWalkAnim。回归保持 `is_eating`，并确认下一帧植物不受伤。
- 历史分析：第 25 项提交 `4ab9544` 把“清除啃食”误写成无头核心语义；第 79 项 `3b96f225` 只修了 no-body/Pogo 分支，普通路径仍未修净。
- 修改文件：`system/damage.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 136. N49：潜水僵尸游泳阶段掉头未在同次伤害中立即死亡

- 问题描述：`snorkel_swim` 在首次掉头后被 PE 留到下一帧，再走额外伤害/普通死亡路径；现于该次 `take_body()` 中直接销毁，正伤、致死伤害和 no-body flag 均不会多存活一帧。
- 依据 / 验证：1051 `UpdateDamageStates@0x52A470` 在 `0x52A52F` 调 DropHead，随后 `0x52A569` 比较 phase `0x3B`，于 `0x52A571` 立即调用 `DieNoLoot`。回归对游泳潜水造成跨掉头阈值伤害并断言同调用 `is_dead`。
- 历史分析：初始提交只在 `zombie_snorkel::update()` 的下一帧补伤；既有潜水修复未覆盖此层级。
- 修改文件：`system/damage.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

## 137. N50：二类饰品销毁错误清零出生最大血量

- 问题描述：`destroy_accessory_2()` 同时把 current HP 和 max HP 清为 0，使铁门、报纸、梯子的出生最大血量原子在脱落后丢失；现只清 current HP 和 type，保留 max HP。
- 依据 / 验证：1051 `TakeShieldDamage` 在 `0x530CA1` 只扣 `[+0xDC]` current HP，归零后 `0x530CB9` 调 `DropShield@0x530A00`；`0x530BAB` 只清 type `[+0xD8]`，未写 `[+0xE0]` max HP。回归让铁门恰好脱落并冻结其 max HP。
- 历史分析：错误来自初始提交 `1a5d0d48`；第 13 项只修了对象池复用时 current HP 残留，并未覆盖 max HP 的销毁语义。
- 修改文件：`system/damage.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e3d92ef5863f6f66eb2f8efaca27775d95c7aca2`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N44-N50 全部 PASS。反编译源码与上述 1051 机器码一致，本轮没有新增反编译项目错误。

## 138. N51：墓碑吞噬者特例仍被冰道和弹坑门禁提前拒绝

- 问题描述：N38 只提前了 Coffee Bean；Grave Buster 仍在通用 ice/crater 棱镜之后，导致 raw grave+ice 或 grave+crater 格无法清墓碑。现先取得 griditem 状态，并让完整 Grave Buster 专支在两项通用门禁前直接返回。
- 依据 / 验证：1051 `Board::CanPlantAt@0x40E020` 在 `0x40E0F4-0x40E115` 对 seed `0x0B` 仅按 normal plant 和墓碑存在性返回；crater 与 `IsIceAt` 到 `0x40E1B9`、`0x40E1E7` 才检查。回归同时构造 grave+ice+crater 并断言允许 Grave Buster。
- 资料冲突：Wiki《墓碑》称冰道覆盖后无法种墓碑吞噬者，这是自然关卡避免该组合的社区概括；与固定 1051 机器码冲突时以二进制为准。该 raw 组合属于 PE 已支持的确定性 setup 原子，不是无尽取舍。
- 历史分析：`94a47a5` 的 N38 修复同类 special-order 问题时只覆盖 Coffee Bean，本项补齐相邻特例。
- 修改文件：`system/plant/plant_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 139. N52：`predict_after()` 未复刻整数中心与 native float 舍入

- 问题描述：共享目标预测 helper 使用浮点半宽、double 速度与 long double 返回值；原版投手、倭瓜和杨桃共同调用的 `ZombieTargetLeadX()` 则以整数半宽，并在速度、减速、位移及最终结果各阶段保持 float。现将 helper 全链改回 float，半宽先做整数 `/2`。
- 依据 / 验证：1051 `ZombieTargetLeadX@0x52A580` 在 `0x52A58D-0x52A5A1` 保存 float 速度/减速，`0x52A5D4-0x52A5DD` 对矩形宽整数除二，`0x52A5E7-0x52A5FA` 的位移及最终差值再次落回 float；调用者分别为投手 `0x46744E-0x46745F`、倭瓜 `0x460B17-0x460B26`、杨桃 `0x45F5BE-0x45F5CA`。回归覆盖减速舍入令倭瓜落点翻转 1px，以及 football 奇数宽中心。
- 历史分析：错误来自初始提交；第 58 项 `6113e0a` 只删除杨桃 caller 重复加半宽，却以该 helper“已返回中心”为前提，未修正中心及精度本身。
- 修改文件：`system/zombie/common_zombie.cpp`、`system/zombie/zombie.h`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 140. N53：Coffee Bean 专用更新函数从未进入 plant dispatch

- 问题描述：`plant_coffee_bean::update()` 已实现 crumble 动画完成即销毁，但类型 switch 没有 Coffee Bean case；旧代码只能等待通用 200cs dead 倒计时，多残留约 130cs。现接入既有 subsystem update，完成时同时清理 coffee map 层。
- 依据 / 验证：1051 `Plant::UpdateAbilities@0x463150` 在 `0x463318-0x463324` 对 seed `0x23` 调 `UpdateCoffeeBean@0x460F60`；后者在 `0x460F60-0x460F8C` 检查 loopCount 后调用 `Plant::Die@0x4679B0`。回归将自然状态设为 work/loop-complete，单次 update 后断言死亡且 `plant_map.coffee_bean==nullptr`。
- 影响：残留对象不只是表现层，还会继续成为投篮车、篮球、蹦极的顶层对象及占据公开 stack 原子。
- 历史分析：更新函数和漏分派 switch 都来自初始提交，此前没有对应修复。
- 修改文件：`system/plant/plant_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 141. N54：`set_is_eating()` 把潜水类型比较写成撑杆相位比较

- 问题描述：末分支比较 `status != pole_valuting_running`，因为两个枚举值恰好同为 `0x0B`，造成潜泳僵尸首次接触时错误重置 eat reanimation，同时撑杆 running 又错误跳过啃食动画。现改为比较 `type != snorkel`。
- 依据 / 验证：1051 `Zombie::StartEating@0x52F250` 在 `0x52F2A1` 读取 ZombieType `[+0x24]` 并与 `0x0B` Snorkel 比较；潜水类型只翻 eating 标志，后续由专属相位处理。回归分别冻结 snorkel 既有 progress 和 pole running 应重置到 eat 动画的双向行为。
- 影响：潜水旧动画 crossing 可在原版同帧触发大蒜/魅惑菇事件，PE 旧逻辑会吞掉该 crossing，因此属于自然战斗时序而非单纯动画。
- 修改文件：`system/damage.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 142. N55：旗帜僵尸普通掉头未清携旗状态原子

- 问题描述：普通 flag 首次掉头后 `has_item_or_walk_left` 仍为 true；现仅在非 `DAMAGE_DOESNT_LEAVE_BODY` 路径清除，no-body 早返仍保留，PogoBreak 分支不变。
- 依据 / 验证：1051 `DropHead@0x529A30` 在 `0x529AF6` 先处理 no-body，普通 flag 于 `0x529C21-0x529C26` 调 `DropFlag@0x529870`，后者在 `0x5298F4` 清 `[+0xBC] mHasObject`。回归覆盖普通清除与 no-body 保留。
- 取舍判断：PE 不需要粒子/贴图，但该字段由 JSON/Python 公开，且 mower 路径本就维护它；恢复布尔原子无需引入表现系统，因此不是必须保留的无尽取舍。
- 修改文件：`system/damage.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 143. N56：`scene` 隐式复制赋值把 `plant_map` 指向源 world

- 问题描述：自定义 copy constructor 会重建 map，但缺少 copy assignment，编译器逐成员赋值会复制 `plant*` 裸指针；`dst.scene = src.scene` 后目标对象池虽已深拷，map 仍指向源池，源释放后形成 UAF。现实现显式赋值，完整复制状态、清空 event sink 并按目标池重建 map；self-assignment 安全。
- 依据 / 验证：这是 PE Legacy/API 自洽问题，原版 Board 无复制接口，因而没有可对应机器码；`pybind.cpp` 把 `World.scene` 暴露为 `def_readwrite`，路径可直接到达。回归按对象 ID 取得目标副本，并断言 map 指针等于目标副本而非源植物；ASan 通过。
- 历史分析：copy constructor 来自 `27cae77`，Python 可写 scene 来自 `372f454`；N18 只强化复制构造，没有覆盖赋值路径。
- 修改文件：`object/scene.cpp`、`object/scene.h`、`tests/peparitytest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

## 144. N57：舞王程序时钟在符号位回绕后使用无符号余数

- 问题描述：PE 以 unsigned 时钟直接 `%`，而原版 AppCounter 是 32 位回绕位模式、舞王取帧使用 signed `idiv`。现保留无符号存储的定义良好回绕，读取时用 `memcpy` 按 int32 位模式解释，再做 signed 余数和除法，避免有符号溢出 UB。
- 依据 / 验证：1051 `GetDancerFrame@0x52DF90` 在 `0x52DFC9-0x52DFD5` 两次使用 signed `idiv`；`UpdateFrames@0x4526E6` 对 `[App+0x838]` 加 1。clock=`0x80000000` 时原版 frame=-16、保持 walking，旧 PE frame=16、进入 armrise2。Wiki《内存基址》与《女仆秘籍》旁证 `+0x838` 为全局舞王相位时钟。
- 影响：自然连续时钟约 248.55 天后出现，也可由 PE 公共场景原子直接恢复；严重度低但证据确定，属于真正的长程无尽偏差。
- 修改文件：`system/zombie/dancing.cpp`、`system/zombie/zombie.h`、`tests/petest.cpp`
- 修改提交号：`4e6bcde079f157fa65b6c19e8ef5b832e9812bae`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N51-N57 全部 PASS。反编译源码与 1051 对应机器码一致，本轮没有新增反编译项目错误。

## 145. N58：投篮篮球速度误用已截整的 projectile 起点

- 问题描述：篮球先把 `z.x+113` 截整写入 projectile，再用该整数 `o.x` 计算水平速度；无目标分支又没有把 `z.x-300` 先截为原版的 int target。现分离 float origin 与 int target，只在创建 projectile 坐标时截 origin，水平/垂直速度保持 float 链。
- 依据 / 验证：1051 `ZombieCatapultFire@0x525730` 在 `0x52573A-0x525744` 保存 float origin，`0x525767-0x525775` 将无目标坐标转 int，`0x5257ED-0x525809` 仅传给 AddProjectile 时截 origin，`0x52580E-0x52585D` 再用原 float origin/int target 算速度。回归以 `z.x=500.5` 覆盖有目标与 null-target 两条。
- 影响：投篮车自然移动后通常处于小数 x；旧误差会改变篮球逐帧位置、保护伞触发与后场受伤帧。
- 历史分析：初始提交遗留，既有投篮目标/弹数修复未涉及 factory 的精度层级。
- 修改文件：`system/projectile/projectile_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

## 146. N59：杨桃跨行索敌遗漏预测左界截整

- 问题描述：旧代码直接以 float `predict ± width/2` 做严格相交，并从未截整的中心计算角度；原版先得到 int 预测左界，横向覆盖和角度中心都复用它。现恢复 float distance/atan2 链，并先生成 `int predicted_left`。
- 依据 / 验证：1051 `FindStarFruitTarget@0x45F470` 在 `0x45F584-0x45F5BA` 逐阶段保存 float 距离，`0x45F5C5` 调目标预测，`0x45F5CA-0x45F5D3` 减整数半宽并截成 int 左界，`0x45F5D7-0x45F622` 复用该值做相交和角度。回归构造右界恰为杨桃中心 80 的跨行梯子；原版不索敌，旧 PE 错把 launch 置 40。
- 历史分析：第 58 项 `6113e0a` 只删除角度重复半宽，N52 `4e6bcde` 只修共享 helper；caller 的截整仍未修净。
- 修改文件：`system/plant/starfruit.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

## 147. N60：僵尸裁剪高度在错误阶段截整

- 问题描述：hit/attack box 使用 `height - int(clipHeight)`，而原版是 `int(height - clipHeight)`；正小数裁剪高度会差 1px。现两处都对整体差值截整。
- 依据 / 验证：1051 hit rect `0x53211A-0x532122` 与 attack rect `0x5321D5-0x5321DD` 都先执行 `fisubr [height]`，再调用 float-to-int。raw 回归以 height=115、rising、dy=0.5、action=40 得 clip=39.5，原版两框高度75，旧 PE为76。
- 取舍判断：自然墓碑出土通常已把高度 round，但该几何原子公开且海豚等分数高度可达；属于低严重度实现机制 bug，不是纯表现取舍。
- 修改文件：`object/zombie.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

## 148. N61：foreign factory `destroy()` 可跨 world 破坏对象

- 问题描述：plant/zombie factory 未验证传入对象属于绑定 scene。foreign plant 会在源对象上写 dead，却按接收 scene 清梯/注销 map；foreign bungee 还能用碰撞 ID 销毁接收 world 的植物或 partner。现任何副作用前用对应 pool 地址所有权校验，foreign 对象抛 `invalid_argument`。
- 依据 / 验证：原版 `Plant::Die@0x4679B0` 和 `Zombie::DieNoLoot@0x530510` 从对象自身携带的 Board 进入，不存在调用者另选 Board。PE 工厂是 C++/Python 公开 API，故需恢复该所有权不变量。双 world 回归覆盖同格梯子和 bungee ID 碰撞，两边均不得变化。
- 历史分析：N20 只修过已删除 facade 的 foreign resolver；自 `372f454` 公开的 factory destroy 仍未覆盖。
- 修改文件：`system/plant/plant_factory.cpp`、`system/zombie/zombie_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

## 149. N62：`world::update_all()` 的 worker 异常直接终止进程

- 问题描述：worker lambda 没有捕获异常；单 world 可正常抛给调用方的 `bad_alloc` 经线程入口逃出后变成 `std::terminate`。现每个 world 使用独占 `exception_ptr` 槽，全部线程 join 后在调用线程 rethrow；线程构造中途失败也会先 join 已启动线程。
- 依据 / 验证：这是 PE batch API 自洽问题，无原版等价接口。回归填满 1024 projectile，再令投篮车在线程中投球：旧版进程以 `0xC0000409` 退出，修复后调用方捕获 `std::bad_alloc`。
- 历史分析：问题来自早期 batch worker；P3 `3b96f22` 只修 worker 数量与输入形状，N19 也未覆盖运行期异常传播。
- 修改文件：`world.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

## 150. N63：C++ observation 仍公开无容量裸指针入口

- 问题描述：P107 只从 pybind 删除 `create(..., float*)`；C++ 头文件仍公开无长度、无 null 校验的写入入口，可稳定写穿短缓冲。仓内外部无合法调用点，现将它移入 private，公开 vector overload 保持不变。
- 依据 / 验证：精确成员签名的 C++17 SFINAE regression 冻结 raw overload 不可公开访问；vector 单个/批量 observation 测试及 ASan 继续通过。
- 历史分析：裸入口由 `c6deee3` 公开，P107 `3b96f22` 只完成 Python 侧隔离，本项补齐 C++ 支持边界。
- 修改文件：`learning/observation_factory.h`、`tests/peparitytest.cpp`
- 修改提交号：`9fc4ca0f4f13834509f82eb28a3ce1e74bcf47c7`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N58-N63 全部 PASS。另发现并修正反编译项目 `Plant::UpdateSquash` 下落曲线错误，见其已修合集第 28 项，代码提交 `8214a34ecd7533c5f0872828aca0550beab1eb51`、记录提交 `f6b387e24053e6d2f20bf500a36a1062adf7c05d`。

## 151. N64：香蒲索敌破坏原版整数距离桶

- 问题描述：Cattail 用 double 精确距离排序，而原版把 `Distance2D` 结果先截成 int 权重，再以严格 `>` 更新；同一整数距离桶应保留先遍历对象。现恢复 int 权重、float 距离后截整，并保留飞行目标 `+10000`。
- 依据 / 验证：1051 `FindTargetZombie@0x4675C0` 在 `0x46784B` 调 Distance2D，`0x467853` 立即转 int，`0x467858-0x467867` 取负/加飞行权重，`0x467873-0x467877` 严格比较。回归构造距离 `sqrt(194)` 与 `sqrt(170)` 的两只僵尸，均落入桶13，断言锁定先分配者。
- 修改文件：`system/plant/plant_base.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`

## 152. N65：Reanimation progress 多一次中间 float 舍入

- 问题描述：旧代码先把 `fps*0.01/n_frames` 转为 float，再加旧 progress；1051 在 x87 中把 increment 与旧值相加后才首次落回 float。现将完整表达式只在最终赋值时截一次。
- 依据 / 验证：1051 `Reanimation::Update@0x471BC0` 在 `0x471BF2-0x471BFB` 算 increment，`0x471BFF` 直接加旧 progress，`0x471C02` 才 `fstp float`。`fps=6,n=10` 连续4次时原版 bits `0x3CC49BA5`，旧 PE 为 `0x3CC49BA6`。
- 影响：这是全体植物/僵尸动画的公共原子；长期可传导到 frame、ground track 与 timed-event 边界。
- 修改文件：`system/reanim.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`
- 后续更正：该项只按指令串推断中间精度，遗漏真实游戏线程运行时的 x87 PC24；现场 Witness 证据与最终修正见第 172 项。

## 153. N66：冰车变速曲线遗漏整数横坐标 age

- 问题描述：PE 直接用 float `z.x` 计算变速；原版先将 mPosX 截为 int，再作为线性曲线 time-age。现先 `int(z.x)`，并保持后续 native float 阶段。
- 依据 / 验证：1051 `UpdateZamboni@0x52A7B0` 在 `0x52A7C7-0x52A7CA` 先转 int，`0x52A7CF-0x52A7E0` 才计算 ratio，`0x52A81E-0x52A82D` 得速度。自然 `x=699.75` 时原版 dx bits `0x3E7F7CEE`，旧 PE 为 `0x3E7FDF3B`。
- 影响：改变冰车移动、碾压与冰道前缘；Wiki《僵尸速度》也明确曲线按整数横坐标逐级变化。
- 修改文件：`system/zombie/zomboni.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`

## 154. N67：巨人投掷小鬼缺少逐阶段 float 落栈

- 问题描述：投掷距离以 double 贯穿到最终写入，漏掉原版在 `x-360`、屋顶修正、随机扣减和 `/3` 后的 float 舍入。现按机器码逐阶段保留 float。
- 依据 / 验证：1051 在 `0x526EE7-0x526EF0`、`0x527032-0x52703C`、`0x52708F-0x527096`、`0x5270FB-0x527112` 分别落回 DWORD，`0x527122` 写小鬼纵速。见证 `x=432.33331298828125` 时原版 d2y bits `0x3F1A4FA3`，旧 PE 为 `0x3F1A4FA2`。
- 历史分析：`9f7069c` 只修屋顶最小距离，`e3d92ef` 只修容量/波次，精度链仍未修净。
- 修改文件：`system/zombie/gargantuar.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`

## 155. N68：公开 factory 创建入口缺少无副作用前置验证

- 问题描述：多个 reference/raw 入口在 Release 中依赖 assert 或根本不校验，非法 type/row/col 可先消费 RNG、分配槽、推进 generation，甚至让玉米炮写出第二格。现统一在任何 RNG/alloc/map 副作用前验证：Plant/Griditem/Projectile reference 非法输入抛 `invalid_argument`；Zombie reference 非法 type 抛错，`try_create` 返回 null，`create_lurking` 返回 false。
- 覆盖边界：Plant 拒绝越界与 col8 Cob；Zombie 拒绝非法 type、lurking 非法 type/行/列；Griditem 仅接受 grave/crater/ladder 及合法格；Projectile raw reference 仅接受 pea..butter 和合法行，仍允许离板 x/y。合法容量的 `bad_alloc`/1023 门禁、任意 spawn_wave 保持不变。
- 验证：回归冻结拒绝返回合同、active/max-used/next-id、RNG 与 plant_map，确认所有非法路径强保证无副作用。
- 历史分析：N16/N25 只修过部分类型域；Python 自 `372f454` 起直接暴露这些 factory，位置和其它对象类型仍未闭合。
- 修改文件：`system/plant/plant_factory.cpp`、`system/zombie/zombie_factory.cpp`、`system/griditem_factory.cpp`、`system/projectile/projectile_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`

## 156. N69：小鬼飞行的纵向校正改变了 float 加法顺序

- 问题描述：旧 PE 用 `(altitude + newY) - oldY` 一次计算并直接把 y 设为 newY；原版先把 `diffY=newY-oldY` 落为 float，再分别执行 `mPosY += diffY` 与 `mAltitude += diffY`。现复刻该两阶段链。
- 依据 / 验证：1051 `UpdateZombieImp@0x5272E0` 在 `0x527326` 取新Y，`0x52732B-0x52732E` 先 `fstp diffY`，`0x527332-0x52733F` 更新位置，`0x527342-0x527350` 再更新高度。与 N67 的原版初速结合后，第72次更新高度 bits `0x36500000` 且仍 flying，第73次才 landing；旧 PE 提前一帧。
- 影响：改变自然小鬼落地帧及横坐标；与巨人精度链共同回归，不能用巨人侧补偿。
- 修改文件：`system/zombie/imp.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`014228cd2dad747e229d7b1381b14a259df0e645`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N64-N69 全部 PASS。Throwing event callback 候选经独立核对为误报：唯一 RSVZ 安装链已在 Rust trampoline 与 runtime ingress 双层 `catch_unwind`，PE 不扩展未承诺的 C++ 吞异常合同。

## 157. N70：随机创建在当前场景无合法行时仍从全零权重选行

- 问题描述：day 的 ducky/snorkel、roof 的 dancing 等有效类型可通过 type 校验，但六行均不合法；旧 `try_create` 仍写 `row_random`、消费 RNG，并从全零 distribution 返回一行。现于任何 row state/RNG 前只读预扫；nullable 返回 null，reference 抛 `invalid_argument`，满池仍为 `bad_alloc`。
- 依据 / 验证：原版自然调用方由 allowed-list 保证类型存在合法行，`PickRowForNewZombie@0x40DC50` 本身没有处理全零公开输入。PE 将随机 create 公开后需补齐这一前置条件。回归冻结 day ducky/snorkel、roof dancing 的 RNG、row_random 与对象池；roof digger 原版/PE 均有 raw 合法行，明确不限制。
- 修改文件：`system/zombie/zombie_factory.cpp`、`system/zombie/zombie_factory.h`、`tests/peparitytest.cpp`
- 修改提交号：`8775aa63c889bdfd56c20e723cab38ef5fbc300b`

## 158. N71：Basketball overload 接受 foreign actor/target

- 问题描述：篮球专用 `create(type,zombie,plant*)` 可用 A world 的投篮车或植物在 B world 分配并瞄准，N61 只修了 destroy ownership。现于读取坐标/分配前验证 zombie 和非空 plant 都属于 factory scene，foreign 输入抛 `invalid_argument`；null target 与同 scene 路径不变。
- 验证：双 world 回归分别替换 actor 和 target，冻结接收 world 的 projectile pool、RNG 及本地对象 HP。
- 修改文件：`system/projectile/projectile_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`8775aa63c889bdfd56c20e723cab38ef5fbc300b`

## 159. N72：磁力菇 double 距离打破原版 float tie

- 问题描述：Magnet 最近目标以 double 完整排序；原版在平方和、sqrt、加行权重三处落回 float，并以严格 `<` 比较，float 相等时保留先对象。现恢复三阶段 float。
- 依据 / 验证：1051 `UpdateMagnetShroom@0x461D90` 在 `0x462126-0x462148` 调 `Distance2D@0x407B10`，其平方和/sqrt 分别落 float；`0x46215B-0x46216C` 加行权重后再落 float，`0x462176-0x46218D` 严格比较。见证两目标 double 距离 302.6499628/302.6499495，但 native float 均为 bits `0x43975332`，应吸走先分配者。
- 历史分析：N9 `3b96f22` 只修 hit-rect 锚点，未修精度层级。
- 修改文件：`system/plant/mushroom_family.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`8775aa63c889bdfd56c20e723cab38ef5fbc300b`

## 160. N73：`get_frame_status()` 在乘法后提前舍入

- 问题描述：PE 先以 float 计算 `progress*(n_frames-1)`，再加 begin；1051 在 x87 中完成乘加后才落 float。现用高精度完成整条乘加，最终只转一次。
- 依据 / 验证：1051 `GetFrameTime@0x472D90` 在 `0x472DBA-0x472DCD` 连续 `fimul frameCount`、`fiadd begin`、`fstp float`。progress bits `0x3EAAAAA9`、n=4、begin=2 时原版 current bits `0x403FFFFF`，frame=2/next=3；旧 PE 得3.0、frame=3。
- 历史分析：N65 只修 progress 推进，未覆盖相邻的 frame extraction。
- 修改文件：`object/common.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`8775aa63c889bdfd56c20e723cab38ef5fbc300b`

## 161. N74：`_ground` 位移与动画 FPS 链多次提前 float 舍入

- 问题描述：ground track 差值、0.01、FPS 连乘每步都以 float 舍入；反向计算 anim FPS 也在 one-over-speed 后的乘法中提前舍入。现按原版保留 distance 和 one-over 两个指定 float 阶段，其余连续链只在末尾落 float。
- 依据 / 验证：1051 `GetTrackVelocity@0x4738D0` 在 `0x473906-0x473917` 完成差值×0.01×fps 后才落栈；`UpdateAnimSpeed@0x52F050` 在 `0x52F1F0-0x52F1F7` 保存 one-over，`0x52F1FB-0x52F20E` 连乘 dx×47 后再落栈。回归位模式：ground 44→45/fps12 原版 `0x3E2C0836`，旧 PE `...35`；dx=.28 的 anim FPS 原版 `0x4146B8A6`，旧 PE `...A7`。
- 修改文件：`object/zombie.cpp`、`system/reanim.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`8775aa63c889bdfd56c20e723cab38ef5fbc300b`
- 后续更正：本项按“末尾才 `fstp`”判断连续乘法只在末尾舍入，遗漏真实游戏线程的 x87 PC24 会在每条算术指令后舍入；其中 ground 44→45/fps12 的 PC24 正确结果是旧实现的 `0x3E2C0835`，N74 标为 native 的 `0x3E2C0836` 实为合并 binary64 结果。`GetTrackVelocity()` 的最终修正见第 181 项。

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N70-N74 全部 PASS。

## 162. N75：屋顶伏兵无合法随机行时先创建半个对象对

- 问题描述：roof `create_lurking(dancing,...)` 可预知 target 没有合法随机行，但旧流程先创建 bungee、消费 RNG/ID，再由 target `try_create` 失败并标死首对象。现于场景分派和任何分配前复用 `has_spawn_row`，失败完全无副作用。
- 验证：回归冻结 roof dancing 的 RNG、row_random、active/max-used/next-id；roof digger 作为正例仍成功创建两只，容量导致第二对象失败时 N44 的 rollback 保持。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`a5ae7cb974caf09468fb4ac4ba443d65ce2dbaa8`

## 163. N76：Timed-event 阈值参数误用 double

- 问题描述：PE `reanim::is_in_progress` 接受 double，而原版 `ShouldTriggerTimedEvent(float)` 的调用实参在入口即为 DWORD；十进制阈值略低于相邻 float 时可漏事件。现声明/定义统一改为 float。
- 依据 / 验证：1051 `ShouldTriggerTimedEvent@0x473B70` 在 `0x473BA3` 从栈 DWORD 读取阈值。`prev=.3f,current=.31f`、调用 `.30000001` 时原版先转成与 prev 相等的 `.3f` 并命中，旧 PE double 值略小而漏掉。
- 历史分析：第34项 `d391f0d` 只修跨循环比较字段，未修参数精度；该 helper 影响海豚、巨人、投篮、大蒜/魅惑等事件。
- 修改文件：`object/common.cpp`、`object/common.h`、`tests/peparitytest.cpp`
- 修改提交号：`a5ae7cb974caf09468fb4ac4ba443d65ce2dbaa8`

## 164. N77：海豚跳跃裁剪区间漏掉 `0.76f` 端点

- 问题描述：上界写成 double `0.75999999`，小于实际 `0.76f=0.759999990463...`，因而把原版包含的端点排除。现使用明确 float `0.76f`。
- 依据 / 验证：1051 GetDrawPos 在 `0x52DD3E` 加载 DWORD `0x3F428F5C` (`0.76f`)，等于端点不跳过。raw progress=.76 时原版 bias=-13、115高碰撞框变128；旧 PE bias=-200、高度115。
- 修改文件：`object/zombie.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`a5ae7cb974caf09468fb4ac4ba443d65ce2dbaa8`

## 165. N78：海豚 `.3/.49` timed event 丢失 `else-if` 关系

- 问题描述：PE 用两个独立 if；raw 单帧跨过两阈值且 `.3` 找不到高坚果时，会继续执行 `.49` 并清 dx。原版首分支一旦命中便跳到函数尾。现第二分支改为 `else if`。
- 依据 / 验证：1051 `UpdateZombieDolphinRider@0x5261E0` 在 `0x526504` 检查 `.3`，命中后 `0x526519` 跳尾；只有为假才到 `0x526589` 检查 `.49`。回归 prev=.2/current=.6、无目标、dx=.5，原版保持 .5，旧 PE 清0。
- 取舍判断：正常动画步长很少同帧跨两阈值，严重度低；但公开 raw 状态和控制流差异确定，修复不引入表现系统。
- 修改文件：`system/zombie/dolphin_rider.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`a5ae7cb974caf09468fb4ac4ba443d65ce2dbaa8`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N75-N78 全部 PASS。第七轮其余 projectile lob、pult、Magnet ladder、撑杆 1 ULP、water/Pogo/Snorkel/Digger 等候选均经 objdump 或穷举证伪为一致、纯表现或无行为见证。

## 166. N79：撑杆高坚果判定的 `0.6f` 严格端点被 double 常量提前放行

- 问题描述：下界写成 double `0.60000002`，略小于实际 `0.6f=0.60000002384...`；progress 恰为 `.6f` 时 PE 错误进入原版严格 `>0.6f` 才允许的撞高坚果窗口。现使用 float `0.6f/0.7f`。
- 依据 / 验证：1051 `UpdateZombiePolevaulter@0x525DC0` 在 `0x525F49` 加载 DWORD `0.6f`，`0x525F72-0x525F7A` 执行严格大于；等于端点应跳过。raw 回归放置攻击框内高坚果，断言 progress=.6f 时仍保持 jumping/action/x。
- 修改文件：`system/zombie/pole_vaulting.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`ecd1c5f631715a36e5702d294955bd22aa26f1aa`

## 167. N80：潜水入水曲线在 `progress*1000` 后提前 float 舍入

- 问题描述：旧 PE 先以 float 计算乘积再转 int；原版在 x87 中以 double 1000 相乘并直接截 int。现先在 double 中完成乘法，再进入既有 float 曲线阶段。
- 依据 / 验证：1051 `UpdateZombieSnorkel@0x526720` 在 `0x5267A4` 载 progress，`0x5267A7-0x5267AD` 乘 QWORD 1000，`0x5267B1` 未落 float 便转 int。progress=.06f 时原版 age59、dy bits `0x3F170A3E`；旧 PE 乘积先舍入60，得到 `0x3F199999`。
- 取舍判断：差异主要是低严重度 raw 高度原子，但机器码和可观察值确定，修复不改变正常状态机合同。
- 修改文件：`system/zombie/snorkel.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`ecd1c5f631715a36e5702d294955bd22aa26f1aa`
- 后续更正：本项只去掉了显式的中间 float store，遗漏真实游戏线程的 x87 PC24 会在 `fmul` 后、转整数前先把寄存器值舍入到24位有效数；`.06f*1000` 的 PC24 正确结果是60、最终 dy 为 `0x3F199999`，N80 标为 native 的 age59/`0x3F170A3E` 实为未按 PC24 舍入的结果。最终修正见第183项。

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；fresh-context 修后复审 N79-N80 全部 PASS。第八轮其余跨轮遗漏检查没有发现新的高可信自然核心问题或反编译错误。

## 168. N81：小丑初始开盒倒计时在除法后提前 float 舍入

- 问题描述：PE 先以 float 完成 `distance/dx`，再转整数；原版 x87 以整数 distance 直接除 float 速度，并在商未落回 float 时截整。现把 distance 提升为 double 后除法，最终才转 unsigned。
- 依据 / 验证：1051 ZombieInitialize 小丑分支在 `0x523003` fild distance，`0x523007` fdiv mVelX，`0x52300A` 直接转 int，再乘2。真实初始化 seed 15193 产生 dx bits `0x3F2992DB`、distance724；原版 countdown2184，旧 PE2186。
- 影响：合法自然随机输出可使命中，开盒相差2cs，属于无尽核心时序。
- 修改文件：`system/zombie/jack_in_the_box.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`a35380b03eb8118bdbd6f0423a20168289893442`

## 169. N82：`sun::add_sun()` 在饱和前发生 unsigned 回绕

- 问题描述：旧实现先算 `current+amount` 再 `min(9990,...)`，大额公开输入会回绕，出现“加阳光反而减1”。现先比较剩余容量，超出即写9990，否则再安全相加。
- 验证：余额500或9990加 `UINT_MAX` 均饱和9990；普通 +25 与 +0 保持。该方法由 C++/pybind 直接公开，是 direct-credit API 自洽问题。
- 修改文件：`system/sun.h`、`tests/peparitytest.cpp`
- 修改提交号：`a35380b03eb8118bdbd6f0423a20168289893442`

本轮统一验证：Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过；修正真实 RNG seed 后 fresh-context 最终复审 PASS。第九/十轮其余随机 objdump 抽样未发现新的 PE 或 canonical decomp 差异。

## 170. N83：冰车冰道前缘在加偏移前提前截整

- 问题描述：旧 PE 先计算 `int(z.x) + 118`；原版先以浮点计算 `z.x + 118.0`，再向零截断为整数。当冰车横坐标为负小数时，旧 PE 的冰道前缘会偏右 1px。
- 依据 / 验证：1051 `UpdateZamboni@0x52A7B0` 在 `0x52A854` 载入 `mPosX`，`0x52A857` 加载 QWORD `118.0`，`0x52A85D` 调转换函数，最终由 `0x6397E5` 的 `cvttsd2si` 向零截断。合法轨迹可到达 `x=-10.003745...`；在 `x=-10.25f` 的最小回归中，原版前缘为107、旧 PE 为108，恰跨过普通场景 C1 的冰道种植门槛。
- 影响 / 取舍判断：旧实现会让第一列约晚 1px（末段约10cs）进入不可种植状态，可错误种下地刺或瞬发灰烬植物；冰道生成与种植限制是无尽核心机制，不是表现层或产品范围取舍。
- 历史分析：错误表达式自初始提交 `1a5d0d4` 存在；`6e504a0` 只修复冰道数组误用列号索引，N66 / `014228c` 只修复冰车速度曲线的整数横坐标，均未覆盖冰道前缘的截断顺序。
- 修改文件：`system/zombie/zomboni.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`5a5128ae7d68b4ce586d8b176faa257208fe5166`

本轮统一验证：修复前 fresh-context 最小测试稳定失败；修复后 Release、Debug 与 MSVC AddressSanitizer 下 `petest`、`peparitytest` 全部通过，fresh-context 独立复审 PASS。达到四小时停止线后未再开启新的检索轮。

## 171. 空或场景不兼容的自然出怪候选会越界取样

- 问题描述：公开 `spawn_flags` 可以在配置阶段被全部清空，或只保留当前场景不能自然生成的类型；旧 `gen_spawn_list()` 随后会用空 `types` 结果索引，形成 C++ 越界未定义行为。现由 `pick_spawn_list()` 在第一次写波表前拒绝缺少稳定合法权重候选的配置，允许调用方继续修改 flags 后重试。
- 覆盖边界：all-false、Day-only-Snorkel 与 Roof-only-Dancing 均返回失败且不进入抽样；混合 flags 中的场景不兼容类型不会进入候选；正常 flags 的既有生成行为不变。
- 修改文件：`system/spawn.cpp`、`system/spawn.h`、`tests/petest.cpp`
- 修改提交号：`5e664fd560e4aee4c2f5374dfec233cf3027646b`

## 172. N84：Reanimation progress 未复刻运行时 x87 PC24 分段舍入

- 问题描述：第 152 项把 `fps*0.01/frame_count+progress` 改为只在最终赋值时转回 float，但原版游戏线程实际以 x87 PC24 执行；`fmul`、`fidiv`、`fadd` 每一步都会先舍入到 24-bit significand。PE 的 x64 binary64 连续表达式因此仍可产生 1 ULP 偏差。现分别在乘、除、加之后显式落回 binary32，不引入 x87 汇编或软件 binary80。
- 依据 / 验证：1051 `Reanimation::Update@0x471BC0` 在 `0x471BF2-0x471C02` 的实际指令依次为 `fld/fmul QWORD/fidiv/fadd/fstp DWORD`。跨后端 Witness 的首只普通僵尸给出真实向量 `progress=0`、`fps bits=0x41233C40`、`frame_count=47`：1051 为 `0x3B0E4203`，x87 PC24 精确复现该值；PC53、PC64 与旧 PE 均为 `0x3B0E4204`。修复后同一 650 帧自然出怪 Witness 的 651 条记录与 1051 逐 bit 相同。
- 影响：公共 reanimation 进度会驱动动画帧、ground track 与 timed-event；本次自然出怪见证在首波生成后连续出现瞬时 bit 差异，虽随后因舍入重合，其他 rate/frame_count 组合可能传播到行为边界。
- 修改文件：`system/reanim.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`8bb5751c469ba637bd0d4888e99be8ecb34cd1db`

## 173. N85：豌豆射手用身体动画循环状态结束头部 shooting hold

- 问题描述：PE 只保存一份 plant reanimation，旧 `update_launch_countdown()` 在豌豆射手的 `shooting` 倒计时降到 0 后，错误读取身体 idle reanim 的 `n_repeated`，并在身体动画已经循环时重置身体动画、提前结束第一次头部 shooting hold。首次修正若在每次射击都重新装载完整头部动画时长，又会忽略原版 head reanim 的 `mLoopCount` 在首次循环后不会由 `SetFramesForLayer()` 清零，导致后续 shooting hold 延后结束。现增加三态的最小头部完成计时：`-1` 表示尚未开始，第一次射击按 25 帧、35 FPS 装载 72 次 update，归零后永久表示 head reanim 已循环；该状态不再修改身体 reanim。
- 依据 / 验证：1051 `FindTargetAndFire@0x45EF10` 通过 `Plant+0x98` 取得 head reanim，并把 `mShootingCounter` 写为 35；`UpdateShooting@0x464820` 的普通 head 分支在 `0x464D15-0x464D1D` 检查该 head reanim 的 `mLoopCount+0x5C`，未完成时于 `0x464D95` 把 shooting counter 写回 1。真实 1051 Full Witness 中，第一轮 shooting 在 frame 150 启动、frame 184 发射、保持 1 到 frame 221 并于 222 归零；第二轮在 frame 327 发射后于 328 立即归零。修复前 PE 分别在 frame 185 提前归零、重置身体 circulation，以及在初版修正中错误保持到 frame 364。
- 影响：`mShootingCounter` 是植物逻辑状态；旧实现还会在错误时点把 canonical body reanimation circulation 从约 `0.103999` 重置到 `0.0048`。修复该 PE 逻辑，并由 RSVZ adapter 将已 disappeared 僵尸的残留 PE reanim 规范为与 1051 已删除 body reanim 相同的 `None` 后，“R3C3 豌豆射手 vs R3 x=780 普通僵尸、Locked 7、停止自然刷怪”的 1051/PE Full Witness 共 2501 帧逐 bit 相同，覆盖多轮射击、子弹飞行/命中和僵尸死亡回收。
- 修改文件：`object/plant.h`、`system/plant/plant_base.cpp`、`system/plant/plant_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`6e4d035401402b654ec945a614578344e6f6ba5a`
- 配套 RSVZ 提交号：`9a2cb9c98ae3de21b542c59e4fd233c09a5e90d1`（PE lineup 场景切换改为原地切换、disappeared zombie reanim canonicalization、微场景默认 Full）。

本轮验证：Release `petest`、`peparitytest` 全部通过；RSVZ `pe-rs` 11 项单测与 4 项 doctest、PE backend 45 项单测与 9 项 doctest 全部通过；同场景 1051/PE Full Witness 2501 帧一致。

## 174. N86：豌豆射手长时间空闲后的第一次 shooting hold 未继承头部 idle 循环状态

- 问题描述：第 173 项以 `-1` 表示头部 shooting 尚未开始，但原版 head reanim 从植物创建时就以 idle 动画推进；`SetFramesForLayer("anim_shooting")` 不会清零既有 `mLoopCount`。因此，头部 idle 已经循环后才第一次遇敌时，原版会在发射后的下一帧清除 `mShootingCounter`，旧 PE 却仍错误等待完整的 72 次 shooting update。现只在第一次开始射击的瞬间读取仍与头部 idle 同步的 body `n_repeated`：已经循环则把最小头部状态直接置 0，否则继续使用独立 72-update 倒计时；射击开始后不再读取 body reanim。
- 依据 / 验证：1051 `FindTargetAndFire@0x45EF10` 在 `0x45F01B-0x45F03A` 对 `Plant+0x98` 的 head reanim 调用 `StartBlend`、写入 play-once-and-hold/rate 并调用 `SetFramesForLayer`，该链路没有清零 `mLoopCount+0x5C`；`UpdateShooting@0x464820` 仍在 `0x464D15-0x464D1D` 直接检查该累计值。真实 1051/PE Full Witness 中，R3C3 豌豆射手等待自然首波到 frame 877 才启动第一次 shooting，frame 911 同时发射；1051 在 frame 912 清零，旧 PE 保持 1 到 frame 948，frame 949 才重新一致。修复后新增“空闲 200 update 后第一次射击”回归，并继续保留第 173 项早期第一次射击和后续射击边界。
- 影响：旧 PE 的子弹发射帧没有因此变化，但在自然首波等长时间无目标场景中错误保留 37 帧 `shooting` 逻辑状态；它会破坏 Full Witness 严格一致性，也错误表达原版 head reanim 的累计循环语义。
- 修改文件：`system/plant/plant_base.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`c7b70fa5080ddc1e10941237be00231d5442689c`

本轮验证：Release `petest`、`peparitytest` 全部通过；“Night、R3C3 豌豆射手、自然普通僵尸显式波表、Locked `0x40000000`、Full 3500 帧”的 1051/PE Witness 共 3501 条记录逐 bit 相同，覆盖首波倒计时、50 只 R3 普通僵尸自然生成、射击、子弹命中、死亡、回收和后续目标受伤。

## 175. N87：雪豌豆仍用 body reanimation 结束 head shooting

- 问题描述：雪豌豆与豌豆射手使用相同的 25 帧 head idle/shooting 动画结构和 35 FPS 射击状态，但第 173、174 项的最小头部循环状态只接入了 `pea_shooter`。因此雪豌豆在长时间空闲后的第一次射击结束时仍落入旧的 body reanim 分支，错误调用 `set_reanim(anim_idle)` 并重置 body circulation。现让 `snow_pea` 与结构相同的单发 `pea_shooter` 共用既有三态，不扩张到具有不同多发/多头状态机的连发、裂荚和三线射手。
- 依据 / 验证：1051 `FindTargetAndFire@0x45EF10` 的 head 分支在 `0x45F01B-0x45F03A` 设置 head shooting；其后只对 Repeater `0x7`、SplitPea `0x1C`、LeftPeater `0x34` 和 GatlingPea `0x28` 改写特殊发射参数，SnowPea `0x5` 保持普通 35 帧路径。`UpdateShooting@0x464820` 在 `0x464D15-0x464D1D` 检查 head `mLoopCount+0x5C`，不应修改 body。真实 Full Witness 中两端均在 frame 911 发出第一枚冰豌豆，但旧 PE 从 frame 912 开始重置 `plants[0].reanim.circulation`，到 frame 3500 仍未重合。
- 影响：冰豌豆生成、命中与减速并未在首差异处改变，但 PE 的雪豌豆 body 动画相位从第一次射击起永久错位，破坏逐帧状态一致性，并可能让依赖 body timed track 的后续行为在边界处偏移。
- 修改文件：`system/plant/plant_base.cpp`、`system/plant/plant_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`bf78721dd75c1d54d6046b5b7bc20fda55cba40c`

本轮验证：Release `petest`、`peparitytest` 全部通过；“Night、R3C3 雪豌豆、自然普通僵尸显式波表、Locked `0x40000000`、Full 3500 帧”的 1051/PE Witness 共 3501 条记录逐 bit 相同。首只冰豌豆 frame 911 发射、frame 1051 命中并写入 1000 帧减速；场景覆盖 50 只 R3 普通僵尸自然生成、冰豌豆飞行/命中、减速计时、减速动画与位移，以及 18 只僵尸受伤。

## 176. N88：平滑选行权重的代数化简改变 float 舍入边界

- 问题描述：PE 将原版 `TodCalcSmoothWeight()` 的 expected/advanced/factor 多步计算代数化简为两个紧凑表达式；两者在实数域等价，但不会在每个原版 float 落点舍入。固定随机值时通常选择相同，逐次变化的自然出怪随机值会在边界上选到不同行。现恢复原版逐步 float 计算和 `0.01..100` 钳制，再交给一次取值的平滑权重抽样。
- 依据 / 验证：1051 `TodCalcSmoothWeight@0x5115C0` 在 `0x5115E8`、`0x5115FE`、`0x511610`、`0x51161C`、`0x511644`、`0x511654` 等位置反复 `fstp DWORD`；`TodPickFromSmoothArray@0x5116B0` 同样逐项落 float，并在 `0x511762` 只调用一次 `Rand(float)`。私有波次 MT 首轮50只普通僵尸测试冻结最终五行 last-picked `{1,3,0,4,2}` 与 second-last `{9,7,6,8,5}`。
- 影响：旧公式保持近似均匀的宏观分布，但给定相同原始随机序列时会改变具体出怪行，阻碍出怪状态机的确定性跨后端验证。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`665d9929ab849372e205a2d435a222a713dc79dc`

## 177. N89：随机浮点与移动动画速率缺少 x87 PC24 中间舍入

- 问题描述：PE 的 `randfloat()` 以 binary64 完成 `next/0x7fffffff*range` 后只落一次 float；原版在除法和乘法之间已按 PC24 舍入。`reanim::update_fps()` 又把 `speed*one_over_speed*47` 合并为一个 binary64 表达式。两处都会让合法随机速度和由其派生的 reanim rate 偶发偏差1 ULP。现分别保存 float unit 与 `speed*one_over_speed` 中间值。
- 依据 / 验证：1051 `MTRand::NextNoAssert(float)@0x5A9A60` 在 `0x5A9A7B` 以 QWORD 常量除法、`0x5A9A81` 乘 float range，并在 `0x5A9A85` 落 DWORD；`Zombie::UpdateAnimSpeed@0x52F050` 在 `0x52F1F0-0x52F1F7` 先计算并保存 one-over-speed，随后于 `0x52F1FF` 乘 `mVelX`、`0x52F202` 乘 QWORD 47，并在 PC24 下逐指令舍入。真实私有波次 Witness 的第7只普通僵尸中，旧 PE speed 为 `0x3EB2F7EB`、1051 为 `0x3EB2F7EC`；修正随机浮点后 speed 一致，旧 PE rate 为 `0x417814A5`、1051 为 `0x417814A4`；分段动画速率乘法后继续一致。
- 影响：差异会传播到 reanimation progress、基于 ground track 的实际位移以及动画事件边界；固定随机常量未必覆盖这些随机向量。
- 修改文件：`system/rng.h`、`system/reanim.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`665d9929ab849372e205a2d435a222a713dc79dc`

本轮验证：Release `petest`、`peparitytest` 全部通过；“Night、显式普通僵尸波表、全局 Locked 7、每波私有 MT base `0x5EED1051`、Full 650帧”的1051/PE Witness 共651条记录逐 bit 相同。首波50只僵尸覆盖五行、26个不同初始 x、50个不同速度和50个不同 reanim rate；全局 Battle RNG 在私有作用域前后保持不变。

## 178. N90：连发 / 多头射手错误重置 body reanimation

- 问题描述：PE 只保存一份 plant reanimation；双重射手、机枪射手和裂荚射手的 shooting countdown 结束时，旧实现把已经循环的 body idle reanim 重置到起点。原版射击运行在独立 head reanimation，body idle track 不应改变。现让这三类 head shooter 在射击收尾时保留 body circulation；既有多发倒计时和裂荚前后头 flags 不变。
- 依据 / 验证：1051 `FindTargetAndFire@0x45EF10` 在 `0x45EF3F` 读取 `Plant+0x94` body reanim，在 `0x45EF7D` 另读 `Plant+0x98` head reanim；`0x45F01B-0x45F03A` 只对 head 调用 `StartBlend` / `SetFramesForLayer`，Repeater 分支 `0x45F042-0x45F08C` 也只改 head rate 与 `mShootingCounter`。`GetPeaHeadOffset@0x465380` 则从 `Plant+0x94` 读取 body transform。真实单株 Witness 中 frame747 两端 body progress 同为 `0x3EF6C838`；frame748 原版继续到 `0x3EF9DAA7`，旧 PE 重置为 `0x3B9D4951`，而两个发射倒计时仍完全一致。第二枚豌豆在 frame865 同时创建，但原版坐标为 `(266.3299866, 90)`，旧 PE 为 `(272.3299866, 92)`。
- 影响：错误 body 相位会改变 `GetPeaHeadOffset()` 得到的枪口坐标，进而改变豌豆逐帧位置、命中边界和伤害时机；不是纯视觉差异。共享收尾分支同时覆盖 Repeater、GatlingPea 与 SplitPea。
- 修改文件：`system/plant/plant_system.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e1eeaa46dbe1069b2e42d8a269aeee7e6a4c4bc2`

本轮验证：Release `petest`、`peparitytest` 全部通过；“Night、R1C3 双重射手、自然普通僵尸显式波表、全局 Locked 7、每波私有 MT、Full 1000帧”的1051/PE Witness 共1001条记录逐 bit相同。原五行混合植物场景修复后首差从 frame865 的双发豌豆坐标推进到 frame2609 的 R5 小喷孢子 flags，证明双发问题已经独立消除。

## 179. N91：短射程蘑菇伤害范围遗漏 `DAMAGES_DOG`

- 问题描述：PE 将小喷菇、海蘑菇、大喷菇、忧郁菇和大嘴花的 damage-range flags 写成仅 `GROUND=0x1`，并误把胆小菇放进同组；原版前五类返回 `GROUND|DOG=0x9`，胆小菇保持默认 `0x1`。现恢复原版分组和 flags。
- 依据 / 验证：1051 `Plant::GetDamageRangeFlags@0x45EB10` 在 `0x45EB6A-0x45EB81` 将 PuffShroom `0x8`、SeaShroom `0x18`、FumeShroom `0xA`、GloomShroom `0x2A`、Chomper `0x6` 汇入 `0x45EBAD` 返回9；ScaredyShroom `0xD` 不在该分支。真实五行混合植物 Witness 在 frame2609 同时创建 R5 小喷孢子，原版 projectile flags 为 `0x9`，旧 PE 为 `0x1`，位置、年龄和运动类型均相同。
- 影响：修正所有使用 `GetDamageRangeFlags()` 的索敌、范围攻击和 projectile 原生状态合同。`DAMAGES_DOG` 在普通冒险僵尸路径通常不改变命中，但不能由 PE 擅自删除或错误赋给胆小菇。
- 修改文件：`object/plant.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e1eeaa46dbe1069b2e42d8a269aeee7e6a4c4bc2`

本轮验证：Release `petest`、`peparitytest` 全部通过；N90/N91 修复后，“Night、R1–R5 C3 分别为双发、机枪、大喷、卷心菜、小喷、自然普通僵尸、每波私有 MT、Full 4000帧”的1051/PE Witness 共4001条记录逐 bit相同。

## 180. N92：冰车变速曲线乘加缺少 PC24 中间舍入

- 问题描述：N66 已让冰车变速曲线使用整数横坐标 age，但最终 `ratio * -0.2 + 0.25` 仍由 PE 合并成 binary64 乘加后一次落 float；1051 在 x87 PC24 下于乘法和加法之间已经舍入。现分别保存 float 乘积和最终和。
- 依据 / 验证：1051 `UpdateZamboni@0x52A7B0` 在 `0x52A7D7-0x52A7E0` 计算 ratio 并 `fstp DWORD`，随后 `0x52A81E` `fmul QWORD`、`0x52A824` `fadd QWORD`、`0x52A82A` 再落 DWORD；游戏线程 PC24 使两条算术指令逐步舍入。真实接触场景的 `curve_age=634` 中，1051 speed bits 为 `0x3E5E3540`，旧 PE 为 `0x3E5E353F`，持续5帧后在下一整数 age 重合；对应位置仍因 float 加法舍入而相同。新增 `x=634.75f` 回归冻结现场向量。
- 影响：合法冰车轨迹会产生短暂1 ULP速度状态差异；本次未传到位置，但其他坐标/累积边界不能依赖偶然重合。修复补全 N66 的原生浮点阶段。
- 修改文件：`system/zombie/zomboni.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`5e664fd560e4aee4c2f5374dfec233cf3027646b`

本轮验证：Release `petest`、`peparitytest` 全部通过；结合 RSVZ signed speed 与 flat-tires 原子映射修正后，“Night、R1橄榄+坚果、R2撑杆+坚果、R3梯子+坚果、R4冰车+地刺、R5巨人+坚果、停止自然刷怪、Full 2500帧”的1051/PE Witness 共2501条记录逐 bit相同。

## 181. N93：`GetTrackVelocity()` 遗漏运行时 x87 PC24 逐指令舍入

- 问题描述：N74 将 ground 差值、`0.01` 与动画 FPS 合并为 binary64 表达式后只在末尾落 float；1051 虽然同样只在末尾 `fstp`，但游戏线程的 x87 PC24 会让 `fsub/fmul/fmul` 每一步先舍入到 24-bit significand。现分别保存 distance 与 scaled 两个 float 阶段，再计算最终速度。
- 依据 / 验证：1051 `GetTrackVelocity@0x4738D0` 在 `0x473906-0x473917` 依次执行 `fld DWORD`、`fsub DWORD`、`fmul QWORD 0.01`、`fmul DWORD fps`、`fstp DWORD`；`UpdateZombieWalking@0x52AAE2-0x52AB3B` 再乘 scale、落 float speed 并更新 `mPosX`。真实私有波次 Witness 的梯子现场为 ground `-30.7f -> -27.9f`、fps `0x41D9318A`、旧 x `0x43DF931C`：旧 PE speed `0x3F429B00`、新 PC24/native speed `0x3F429AFF`，下一帧 x 分别为 `0x43DF31CE/0x43DF31CF`。
- 影响：所有依赖 `_ground` track 的僵尸都可能在特定动画帧产生实际坐标差异，并可传播到整数坐标、碰撞和动作边界；本次首先由自然梯子场景在啃蒜前暴露。
- 修改文件：`object/zombie.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`2787b189915f8d3a0c97bd186b4768d9fbf0eee7`

本轮验证：Release、Debug 的 `petest`、`peparitytest` 全部通过；“Night、五行 C4 高耐久大蒜、五行 C1 高耐久火炬树桩、自然普僵/橄榄/梯子显式波表、全局 Locked 7、每波私有 MT、Full 5000帧”的1051/PE Witness 共5001条记录逐 bit相同。场景覆盖17只普僵的170帧换行及33只橄榄、33只梯子的21帧换行。

## 182. N94：气球与跳跳初始化过早应用类型 y 偏移

- 问题描述：PE 在 `zombie_base::init()` 开头先写具体 type，再调用 `zombie_init_y()`，因此气球和跳跳出生时已经减去30/16像素。1051 在计算基础行 y 后才写 `mZombieType`，后续位置更新再以每帧最多1像素逐步靠近类型目标 y。现把 PE 的 type 写入移动到初始 y 计算之后。
- 依据 / 验证：1051 `ZombieInitialize@0x522580` 在 `0x5225B7` 调用 `GetPosYBasedOnRow@0x531880`、`0x5225BF` 写 `mPosY`，到 `0x5225E6` 才写 `mZombieType`；`UpdateZombiePosition@0x52ADB0-0x52AE2D` 每帧最多调整1像素。真实冰菇 Witness 的 Night R4 气球在1051中从 `y=350` 下降到 `320`，旧 PE 直接从 `320` 开始，差异持续frame0..29；跳跳同理应从基础 y 经过16帧抵达偏移目标。
- 影响：出生后最初30/16帧的 y、整数 y 与碰撞框错误，可能改变早期子弹碰撞、植物索敌和其他基于矩形的动作边界；与第100帧才生效的冰菇逻辑无关。
- 修改文件：`system/zombie/common_zombie.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`e55240d3f546433f6a78bac834b8522ecefbb780`

本轮验证：Release、Debug 的 `petest`、`peparitytest` 全部通过；“Night、帧0醒着的寒冰菇、固定普僵/橄榄/梯子/投篮/巨人/气球/跳跳/地下矿工/冰车、停止自然刷怪、全局 Locked 7、Full 2300帧”的1051/PE Witness 共2301条记录逐 bit相同。场景同时覆盖普通冻结伤害、气球/跳跳仅减速、矿工/冰车完全免疫，以及气球30帧和跳跳16帧初始 y 过渡。

## 183. N95：潜水入水 `progress*1000` 遗漏运行时 PC24 舍入

- 问题描述：N80 将 `progress*1000` 改成 binary64 乘法后直接转整数；1051 在内存中虽不保存中间 float，但 x87 PC24 会在乘法指令后先舍入到24位有效数。现把乘积显式落回 float 后再截断为整数，随后保留原有线性曲线阶段。
- 依据 / 验证：1051 `UpdateZombieSnorkel@0x5267A4-0x5267B1` 依次执行 `fld DWORD progress`、`fld QWORD 1000`、`fmul`，再调用转整数 helper；真实 Pool Witness 的 progress bits `0x3EA3D70A` 在 PC24 下乘1000得到320，旧 PE binary64 结果为319.9999928并截成319，入水高度分别为 `0x404CCCCC` 与 `0x404C28F6`，差异持续1帧。
- 影响：潜水入水高度、整数/碰撞几何可在合法自然动画向量上相差0.01；本次状态切换未改变，但不能依赖偶然边界。
- 修改文件：`system/zombie/snorkel.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`40856c2f6d353573fd7e1a60e5ff0f25d999590c`

## 184. N96：海豚跳跃高度代数化简改变曲线舍入阶段

- 问题描述：PE 将原版 `TodAnimateCurveFloat(120,0,countdown,0,10,LINEAR)` 化简成 `(120-countdown)/12`；实数等价，但原版先把 `(countdown-120)/(0-120)` 落 float，再乘10。现恢复 warped-age 与位置计算两个 float 阶段。
- 依据 / 验证：1051 `UpdateZombieDolphinRider@0x5264E3-0x5264F0` 调用 `TodAnimateCurveFloat@0x511BF0`；后者在 `0x511C0E-0x511C1E` 执行整数除法并 `fstp DWORD`，再调用线性曲线。countdown115时 native 高度为 `0x3ED55556`，旧 PE `/12` 为 `0x3ED55555`；真实 Pool Witness 共27个跳跃帧出现1 ULP高度差，但阶段切换帧相同。
- 影响：海豚跳跃高度和碰撞框在合法自然状态下产生离散1 ULP差异，极端几何边界可能进一步影响命中或目标判断。
- 修改文件：`system/zombie/dolphin_rider.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`40856c2f6d353573fd7e1a60e5ff0f25d999590c`

本轮验证：Release、Debug 的 `petest`、`peparitytest` 全部通过；“Pool、R3海豚依次跨越C6坚果并啃食C3坚果、R4潜水在C5坚果上浮啃食、禁用自然刷怪、稳定天降阳光52/425、全局 Locked 7、Full 3500帧”的1051/PE Witness 共3501条记录逐 bit相同。海豚阶段51→52→53→54→55与潜水阶段57→58→59→60→61全部覆盖。

## 185. N97：`place_zombie()` 提前同步整数坐标缓存

- 问题描述：PE 的 `try_create_in_grid()` 在重定位 `x/y` 后立即同步 `int_x/int_y`；原版 `Challenge::IZombiePlaceZombie()` 只写浮点 `mPosX/mPosY`，整数 `mX/mY` 要到下一次 Zombie update 才同步。现删除这两次提前写入，普通僵尸与蹦极都保留构造器缓存一帧。
- 依据 / 验证：1051 `IZombiePlaceZombie@0x42A0F0` 对蹦极写 target col、row、`mPosX/mPosY`，对其他僵尸只写 `mPosX`，均不写整数缓存。R1C8/R3C5 两只定点蹦极的 Full Witness 在 frame0 首差为 `int_position.x`：1051 保留构造值、旧 PE 已写目标格坐标，frame1 更新后自然重合。
- 影响：所有 `place_zombie()` 对象的 frame0 整数坐标及依赖它的碰撞/观测状态可能提前一帧；手动蹦极定点夹具首先暴露该差异。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/petest.cpp`
- 修改提交号：`0c457ac55b2717f7f5b572469d53213990b7208a`

## 186. N98：伴舞生成坐标遗漏舞王 `mPosX` 到局部整数的截断

- 问题描述：PE 在 `create_backup_dancers()` 中直接用舞王 float `x` 及 `x±100` 创建四只伴舞；原版先把 `mPosX` 赋给局部 `int aPosX`，再传入 `SummonBackupDancer()`。现先截断舞王 x，再计算左右偏移。
- 依据 / 验证：1051 `SummonBackupDancers@0x528970` 的局部 `aPosX` 为 int；真实 Full Witness 在 frame349 首次召唤时，舞王 x 为 `533.3747`，四只伴舞原版 x 分别为 `533/533/433/633`，旧 PE 均多出 `0.3747`，整数坐标、行和关系已相同。
- 影响：伴舞整段队形移动、子弹命中和补召时机都会继承初始小数偏移；本次 R2 机枪射手持续攻击上方伴舞的场景从首次召唤起不再重合。
- 修改文件：`system/zombie/dancing.cpp`、`tests/petest.cpp`
- 修改提交号：`982aac33e169c46e51fc7a3f33a9b10df880244d`

## 187. N99：在场伴舞初始化错误切换到 `anim_armraise`

- 问题描述：PE 的 `zombie_backup_dancer::init()` 无条件把 reanimation 切为 `anim_armraise`；原版 `ZombieInitialize()` 只对不在 Board 上的展示对象播放该动画，真实召唤伴舞保留默认帧段。现删除在场模拟中的错误覆盖。
- 依据 / 验证：1051 `ZombieInitialize` 的 Backup Dancer 分支以 `!IsOnBoard()` 为动画切换门禁；真实 Full Witness 在 frame349 首次召唤时，原版四只伴舞均为 frame start0/count21、anim time `0x3BF252A2`，旧 PE 为 start21/count11、anim time `0x3C32BC0B`，而 phase/counter/rate/关系已一致。
- 影响：伴舞出土后动画进度、ground track 位移和后续队形状态持续偏移，可能进一步改变坐标、受击与补召时机；不是纯表现差异。
- 修改文件：`system/zombie/backup_dancer.cpp`、`tests/petest.cpp`
- 修改提交号：`982aac33e169c46e51fc7a3f33a9b10df880244d`

## 188. N100：伴舞出土结束产生负零高度

- 问题描述：PE 用 `round(countdown * -4/3)` 计算伴舞出土高度；countdown=0 时 IEEE-754 结果是 `-0.0f`。原版整数线性曲线在终点返回0，再写入 float 高度，得到 `+0.0f`。现对终点显式写正零。
- 依据 / 验证：N99 修复后的真实 Full Witness 首差推进到 frame498，伴舞其他状态均一致，1051 altitude bits 为 `0x00000000`，旧 PE 为 `0x80000000`；下一帧已进入正常队形状态。
- 影响：不改变数值比较或动作分支，但破坏浮点状态的逐 bit 合同，并可能传播到保留符号零的序列化/计算。
- 修改文件：`system/zombie/backup_dancer.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`982aac33e169c46e51fc7a3f33a9b10df880244d`

## 189. N101：每波私有MT错误覆盖完整僵尸构造器

- 问题描述：确定性测试的 `wave_spawn_random` 原先在整个 `spawn_current_wave_objects()` 内替换 Battle RNG，导致1051独有的声音、粒子和动画随机调用推进同一私有流，后续僵尸的选行、位置、速度和倒计时随表现层实现发生偏移。现仍按 `base_seed + zero_based_wave` 建立每波私有MT，但只在逐僵尸选行原子过程内启用；构造器继续使用全局 Locked。PE相应删除仅为完整构造序列补充的 `Rand(5)` variant 与 `RandRangeInt(300,400)` groan空消费。
- 依据 / 验证：PE24 Full Witness 在frame0..598逐bit一致，旧完整作用域于frame599首次出怪后分歧。冰车生成末尾的 `FOLEY_ZAMBONI` 在1051中额外消耗随机音高和单样本选择两个MT值，使下一只海豚初始x为798而PE为795；仅隔离声音后，海豚恢复为双方795，但矿工创建隧道粒子后最终速度仍分歧，证明不能靠补单个声音调用修复。原版 `SpawnZombieWave` 的三个选行调用点为`0x412F7B/0x412FEA/0x413050 -> PickRowForNewZombie@0x40DC50`。
- 影响：默认关闭的PE宏观测算随机语义不变；Locked Witness仍能得到每波自然分散的行选择，但初始位置、速度和类型专属倒计时保持固定，不再受PE未建模表现层影响。需要这些属性变化时应增加按僵尸/用途派生的独立随机作用域。
- 修改文件：`object/scene.h`、`object/scene.cpp`、`system/spawn.cpp`、`system/zombie/zombie_factory.cpp`、`system/zombie/common_zombie.cpp`、`tests/peparitytest.cpp`、`AGENTS.md`
- 修改提交号：`bdbab3d`

本轮验证：Release `petest` 与 `peparitytest` 通过；RSVZ PE24 row-only Witness 完成1关并生成complete artifact。真实1051首次验证暴露row-pick shim未转发栈参数并闪退，修正为复制ZombieType参数且最终`ret 4`后，开启Aggressive跳帧约4秒完成1关并生成complete artifact。两端首差从frame599推进到frame14912的末波特殊生成坐标，frame14943重合；其后到1051轮次边界前769帧逐bit一致。

## 190. N102：珊瑚三人组候选格顺序与权重尺度错误

- 问题描述：PE 的 `pool_lurking_spawn()` 按“3路6–9列，再4路6–9列”排列8个候选格，并给每格权重1；1051 `SpawnZombiesFromPool()` 按“逐列的3路、4路”建立数组且每格权重10000。普通随机下权重同比缩放不改变边际分布，但固定返回值会映射到不同序号。现同时恢复原生列优先顺序和10000权重。
- 依据 / 验证：Wiki记录珊瑚三人组在W20刷新200cs后于水路6–9列生成；1051 `SpawnZombiesFromPool@0x4128F0` 的循环为外层`grid_x=5..8`、内层`grid_y=2..3`，并逐项写`mWeight=10000`。PE24 row-only Witness 在frame14912出现普通僵尸 `RisingFromGrave` 首差，旧PE第三只为3路8列附近`x=575`，1051为3路7列附近`x=495`。第一次只修顺序后，Locked 7仍因`7 % 8`在权重1数组选末项、而在10000权重数组选首项而不一致，证明权重尺度也是固定随机合同的一部分。
- 影响：修正泳池/雾夜关末珊瑚三人组的格点分布与确定性映射；默认随机下边际分布仍均匀，但固定seed/Locked测试以及具体关末受击、啃食和清场时机此前会不同。
- 修改文件：`system/spawn.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`bdbab3d`

## 191. N103：珊瑚出水未重新进入泳姿reanimation

- 问题描述：PE 的 `create_pool_or_night_lurking()` 重定位珊瑚僵尸并设置 `rising_from_ground/is_in_water` 后，没有执行原版 `RiseFromGrave()` 中的 `StartWalkAnim(0)`。现复用既有 `reanim.update_status()`，按当前水中状态选择 `anim_swim`并重置进度。
- 依据 / 验证：1051 `Zombie::RiseFromGrave@0x531C90` 在设置 `mInPool=true`、50cs出水倒计时后调用 `StartWalkAnim(0)`；PE24修正格点后，Full Witness唯一剩余差异为frame14912..14942同一只珊瑚的 `reanim.anim_time`，实际坐标、状态、倒计时已一致。该31帧差异持续到对象被击杀。
- 影响：除动画进度外，泳姿track还参与后续reanimation速度/位移语义；珊瑚不应继承普通构造器完成时的旧动画进度。
- 修改文件：`system/zombie/zombie_factory.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`bdbab3d`

本轮最终验证：Release `petest`、`peparitytest`通过；PE24 Full Witness在排除两端均不派发的500cs白字/选卡传输后，PE与1051均为complete、15712帧，`witness-diff`报告`witnesses match across 15712 frames`。

## 192. N104：Locked 模式错误冻结跨轮 Level seed

- 问题描述：PE 在生存轮结束时仅在 Level RNG 未锁定时执行 `source_seed + 101` 重播种，导致 Locked Witness 的固定返回值连同 seed 元数据一起被冻结；1051 仍会推进下一轮的 Level seed，只保留 Locked 的固定返回值。现无条件推进 Level seed，并在原流为 Locked 时恢复同一 fixed value 与锁定状态。
- 依据 / 验证：三轮 PE24 Witness 在清除1051选卡预览残留后，第二轮首帧首差推进到 Level RNG seed；新增回归覆盖 `11 -> 112`，同时保持 `locked=true`、`fixed=7`、`next()=7`。
- 影响：修复 Locked 场景的跨轮随机元数据和未来解除锁定后的 Level 流起点；非 Locked 行为不变。
- 修改文件：`world.cpp`、`tests/peparitytest.cpp`
- 修改提交号：`bdbab3d`

本轮 Witness 基础设施同步：`obj_list::reset_empty_allocator()` 仅在 active count 为0时归零 high-water/free-list，保留 generation key；RSVZ 用它消除1051选卡预览僵尸对下一轮空池槽位顺序的影响，非 Witness 的PE模拟流程不会调用。`petest`覆盖非空拒绝、空池归一、槽0复用和 generation 继续递增。

本轮最终验证：Release `petest`、`peparitytest`通过；PE24 三轮 Survival Digest Witness 与真实1051均为complete、47136帧，CI比较报告`witnesses match across 47136 frames`。

## 193. 香蒲攻击标志漏掉 0x08

- 问题描述：`plant::get_attack_flags()` 的香蒲分支返回 `0x03`，缺少原版 `0x0B` 中的 `0x08`。该值也写入香蒲刺，造成 Full Witness 状态分歧。
- 依据：pe96 三后端对比的固定值 7 在第 702 帧首次产生香蒲刺时，PE 标志为 3、1051 为 11；原版 `Plant::GetDamageRangeFlags` 在 `0x45EB83` 比较植物类型 `0x2B`，`0x45EB88` 明确返回 `0x0B`。
- 修正：香蒲与已有蘑菇分支一样保留 `attack_flags::dog` 位，不改变其它植物的标志。该位补齐不代表已消除香蒲追踪计算的其它浮点差异。
- 验证：新增香蒲标志回归；Release `petest`、`peparitytest` 全部通过；800 帧 Full 重放中第 702 帧差异消失，下一差异是第 707 帧速度的 1 ULP 差异。完整实验见 RSVZ `target/pe96-witness-20260914/`。
- 修改提交号：`0546a84`。

## 194. 投篮车待机动画错误停留在末帧

- PE 在投篮结束、有剩余篮球时以 `once` 启动 `anim_idle`；1051 `0x525ADF` 传入循环类型 0。Full Witness 固定值 7 在第 1526 帧首次记录到 3/0 差异。
- 修正仅将这一参数改为既有 `reanim_type::loop`，保留待机倒计时和投篮逻辑。动画进度、循环方式是 RSVZ 公开事实，不能仅从 Witness 屏蔽。
- 验证：待机状态回归通过；后续 10 轮 Full 的首差已越过该帧。
- 修改提交号：`0546a84`。

## 195. 香蒲追踪与投手初速度的浮点舍入顺序

- 香蒲：1051 `Normalize@0x46EE80` 把平方根存为 float 后才做除法；PE 使用 `auto` 保存 double 平方根。原版 `0x46D9B1..0x46D9F4` 先计算 age × double(0.001f)，再乘归一方向、加旧速度；PE 原先从方向 × age 开始连乘。目标中心的加减也按原版先求中心再相减。现明确保留对应 binary32 中间结果。
- 投手：1051 `0x467536/0x46753A` 的 PC24 除法、减法分别舍入；PE 原先到表达式末尾才转 float。现在 `range_y / 120` 后增加一次 float 舍入。
- 冰车：校正既有分步计算中 double 常量的末位，使字面量精确对应 EXE `0x679728`。
- 验证：香蒲第 706→707 帧、冰瓜第 2971 帧的真实 1051 数值回归通过；不依赖 x87、全局浮点模式或平台特定汇编。
- 修改提交号：`0546a84`。

## 196. 有头状态与死亡状态混用

- 第 3389 帧死亡巨人的 HP、phase、动画等已一致，但 RSVZ 的 `zombie_has_head` 从 PE `is_not_dying` 读取，返回 false；1051 的 `mHasHead` 仍为 true。
- 原版 `DropHead@0x529A30` 独立清除有头状态；PE 的 `is_not_dying` 同时用于头部丢失和完整尸体死亡，不能提供同一语义。
- 修正：对象中增加独立 `has_head`，构造为 true、实际掉头时置 false；旧 AI/伤害分支仍使用原字段。RSVZ getter 改为读取该原子事实。未增加派生查询缓存或新状态机。
- 验证：普通掉头、有头巨人死亡、后端读取独立于死亡标志的回归通过；完整 Witness 验证继续使用固定值 7。
- 修改提交号：`0546a84`。

## 197. 保持动画速度的零参数重复应用减速

- Full Witness 固定值 7，第 11680 帧舞王动画速率为 PE 4.5、1051 9，之后动画进度持续不同。
- `reanim::set` 在参数 fps 为 0 时保留旧的有效速率，却随后把该有效速率覆盖到基础 `prev_fps`；再次套用减速就重复减半。原版 `PlayZombieReanim@0x528B49..0x528B6F` 仅在输入参数非零时更新 `mOriginalAnimRate`。
- 修正：基础速率仅从非零参数写入，再统一调用一次 `update_fps`。这修复共用函数，不给舞王添加特例。
- 验证：减速中传零切换动画仍保持半速，解除减速后恢复原始速率；Release `petest`、`peparitytest` 通过。完整实验见 RSVZ `target/pe96-witness-20260914/reanim-fixed-7/`。
- 修改提交号：`0546a84`。

## 198. 蹦极候选格子按行而非按列排列

- 固定值 7 的 Full Witness 在第 19104 帧、第一轮 W10，第三个蹦极的目标从原版 3-1 变成 PE 1-3；此前状态已一致。
- 原版 `PickBungeeZombieTarget@0x5246A0` 按列再按行枚举；`0x524853..0x524878` 先完成行计数，再递增列。PE 原先相反。相同权重下的边际概率不因排列改变，但固定值与实际种子的目标映射不同。
- 修正：仅交换候选格子循环的嵌套顺序，保留原权重和排除条件。
- 验证：锁定 7、先后两个蹦极选择 1-1 和 2-1 的回归通过；Release `petest`、`peparitytest` 通过。完整实验见 RSVZ `target/pe96-witness-20260914/bungee-fixed-7/`。
- 修改提交号：`0546a84`。

## 199. 篮球伤害事件将普通植物0血误报为死亡（待提交）

- 问题：篮球将300血瞬时植物打到0血时，finish_effect报告Killed，导致观察者提前清除待生效记录；植物本身仍可正常触发。
- 修正：篮球0血报告HpDelta；负血或实际销毁仍报告Killed，保留啃食等来源的0血即时销毁语义。只改事件分类，不改植物伤害或更新机制。
- 二进制证据：1051 Plant::Update的0x463ED7比较health与0，0x463EDB以jge跳过Plant::Die，即普通更新仅在health<0时销毁。对应反汇编保存于六二脚本target/logs/clearance-fixes-20261023/plant-update-disassembly.txt。
- 验证：petest新增篮球75→0仍存活且为HpDelta、0→-75报告Killed的回归；Release petest与peparitytest通过。
