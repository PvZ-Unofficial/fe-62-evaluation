//! Entity borrows bounded by one shared backend borrow.
use crate::live_value::read_or_abort;
use rsvz_backend_api::{
    BoardStateBackend, GridItemEditBackend, GridItemReadBackend, GridItemStateBackend, ItemClickCollectBackend,
    ItemReadBackend, PlantContactBackend, PlantPlacementBackend, PlantReadBackend, PlantRemoveBackend,
    PlantStateBackend, ProjectileReadBackend, SeedBankReadBackend, SeedCooldownReadBackend, SeedPacketBackend,
    SunQueryBackend, ZombieContactBackend, ZombieKillBackend, ZombieRawFactsBackend, ZombieReadBackend,
    ZombieRemoveBackend, ZombieStateBackend, ZombieVerticalPositionBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{
    CardSelection, ContactRect, DamageRangeFlags, Grid, GridItemId, GridItemKind, ItemId, ItemKind, PlantId, PlantKind,
    PlantWeapon, ProjectileId, SeedSlot, ZombieId, ZombieKind,
};

/// A protected backend borrow. Creating a Frame does not query Board availability.
///
/// Game examples and borrow failures are checked with
/// `cargo test -p rsvz-game --features backend-tests,rsvz-current/pvz-emulator --doc`.
///
/// ```no_run
/// # #[cfg(feature = "backend-tests")]
/// # {
/// rsvz_game::timeline::at_frame(1, 401, |frame| {
///     for zombie in frame.zombies() {
///         if zombie.hp() < 100 { zombie.remove(); }
///     }
/// });
/// # }
/// ```
///
/// ```compile_fail
/// use std::cell::RefCell;
/// thread_local! { static SAVED: RefCell<Option<rsvz_game::Frame<'static>>> = const { RefCell::new(None) }; }
/// rsvz_game::timeline::at_frame(1, 401, |frame| SAVED.with(|saved| *saved.borrow_mut() = Some(frame)));
/// ```
///
/// ```compile_fail
/// rsvz_game::timeline::at_frame(1, 401, |frame| {
///     std::thread::scope(|scope| { scope.spawn(move || frame.zombies().count()); });
/// });
/// ```
pub struct Frame<'frame> {
    backend: &'frame CurrentBackend,
}

/// Runs a callback with a short shared borrow; entity references cannot escape it.
/// Access faults abort the current callback through the normal operation boundary.
///
/// ```compile_fail
/// let plant = rsvz_game::with_frame(|frame| frame.plants().next());
/// ```
///
/// ```compile_fail
/// rsvz_game::with_frame(|frame| {
///     std::thread::scope(|scope| { scope.spawn(move || frame.zombies().count()); });
/// });
/// ```
///
/// ```compile_fail
/// rsvz_game::with_frame(|frame| {
///     let mut zombies = frame.zombies();
///     rsvz_game::tick::on_frame(move |_| { zombies.next(); });
/// });
/// ```
pub fn with_frame<R>(callback: impl for<'frame> FnOnce(Frame<'frame>) -> R) -> R {
    crate::access::with_backend(|backend| callback(Frame { backend }))
}

impl Frame<'_> {
    /// Finds the first matching live plant by native pool index at an anchor grid.
    pub fn plant_at(&self, grid: Grid, mut matches: impl FnMut(&PlantRef<'_>) -> bool) -> Option<PlantRef<'_>>
    where
        CurrentBackend: PlantReadBackend,
    {
        let mut found: Option<PlantRef<'_>> = None;
        read_or_abort(
            self.backend.for_each_plant_at_anchor_grid(grid, |handle| {
                let plant = PlantRef {
                    backend: self.backend,
                    handle,
                };
                if matches(&plant) && found.as_ref().is_none_or(|old| plant.id().index() < old.id().index()) {
                    found = Some(plant);
                }
                Ok(())
            }),
            "failed to find plant at grid",
        );
        found
    }

    /// Visits live plants in the backend's native anchor-grid order.
    pub fn for_each_plant_at_anchor_grid(&self, grid: Grid, mut visitor: impl FnMut(PlantRef<'_>))
    where
        CurrentBackend: PlantReadBackend,
    {
        read_or_abort(
            self.backend.for_each_plant_at_anchor_grid(grid, |handle| {
                visitor(PlantRef {
                    backend: self.backend,
                    handle,
                });
                Ok(())
            }),
            "failed to visit plants at grid",
        );
    }

    pub fn sun(&self) -> u32
    where
        CurrentBackend: rsvz_backend_api::SunQueryBackend,
    {
        read_or_abort(self.backend.sun(), "failed to read sun")
    }

    pub fn spawn_allowed(&self, kind: ZombieKind) -> bool
    where
        CurrentBackend: rsvz_backend_api::BoardStateBackend,
    {
        read_or_abort(self.backend.spawn_allowed(kind as u32), "failed to read spawn types")
    }

    pub fn can_plant_at(&self, kind: PlantKind, grid: Grid) -> bool
    where
        CurrentBackend: rsvz_backend_api::PlantPlacementBackend,
    {
        let selection = read_or_abort(
            rsvz_model::CardSelection::Plant(kind).checked(),
            "invalid planting kind",
        );
        read_or_abort(self.backend.can_plant_at(selection, grid), "failed to check planting")
            == rsvz_model::Plantability::Allowed
    }

    pub fn plants(&self) -> impl Iterator<Item = PlantRef<'_>> + '_
    where
        CurrentBackend: PlantReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.plants(), "failed to iterate plants").map(move |handle| PlantRef { backend, handle })
    }
    pub fn zombies(&self) -> impl Iterator<Item = ZombieRef<'_>> + '_
    where
        CurrentBackend: ZombieReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.zombies(), "failed to iterate zombies").map(move |handle| ZombieRef { backend, handle })
    }
    pub fn plant(&self, id: PlantId) -> Option<PlantRef<'_>>
    where
        CurrentBackend: PlantReadBackend,
    {
        read_or_abort(self.backend.plant(id), "failed to resolve plant").map(|handle| PlantRef {
            backend: self.backend,
            handle,
        })
    }
    pub fn zombie(&self, id: ZombieId) -> Option<ZombieRef<'_>>
    where
        CurrentBackend: ZombieReadBackend,
    {
        read_or_abort(self.backend.zombie(id), "failed to resolve zombie").map(|handle| ZombieRef {
            backend: self.backend,
            handle,
        })
    }
}

/// A plant slot that remains address-valid until the enclosing Frame borrow ends.
///
/// ```compile_fail
/// use std::cell::RefCell;
/// thread_local! { static SAVED: RefCell<Option<rsvz_game::PlantRef<'static>>> = const { RefCell::new(None) }; }
/// rsvz_game::timeline::at_frame(1, 401, |frame| {
///     SAVED.with(|saved| *saved.borrow_mut() = frame.plants().next());
/// });
/// ```
pub struct PlantRef<'frame>
where
    CurrentBackend: PlantReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as PlantReadBackend>::PlantHandle<'frame>,
}
impl PlantRef<'_>
where
    CurrentBackend: PlantReadBackend,
{
    pub fn id(&self) -> PlantId {
        self.backend.plant_id(self.handle)
    }
    pub fn hp(&self) -> i32 {
        self.backend.plant_hp(self.handle)
    }
    pub fn kind(&self) -> PlantKind {
        read_or_abort(self.backend.plant_kind(self.handle), "failed to read plant kind")
    }
    pub fn raw_kind(&self) -> PlantKind {
        read_or_abort(
            self.backend.plant_raw_kind(self.handle),
            "failed to read raw plant kind",
        )
    }
    pub fn is_squished(&self) -> bool {
        self.backend.plant_is_squished(self.handle)
    }
    pub fn is_sleeping(&self) -> bool {
        self.backend.plant_is_sleeping(self.handle)
    }
    pub fn state(&self) -> i32 {
        self.backend.plant_state(self.handle)
    }
    pub fn grid(&self) -> Grid {
        crate::plant::grid_from_handle(self.backend, self.handle)
    }
    pub fn is_alive(&self) -> bool {
        self.backend.plant_is_alive(self.handle)
    }
    pub fn remove(&self)
    where
        CurrentBackend: PlantRemoveBackend,
    {
        read_or_abort(self.backend.remove_plant(self.handle), "failed to remove plant");
    }
}

/// A zombie slot that remains address-valid after death until the Frame borrow ends.
///
/// ```compile_fail
/// rsvz_game::timeline::at_frame(1, 401, |frame| {
///     let mut zombies = frame.zombies();
///     rsvz_game::tick::on_frame(move |_| { zombies.next(); });
/// });
/// ```
///
/// ```compile_fail
/// rsvz_game::timeline::at_frame(1, 401, |frame| {
///     let zombie = frame.zombies().next().unwrap();
///     std::thread::scope(|scope| { scope.spawn(move || zombie.hp()); });
/// });
/// ```
pub struct ZombieRef<'frame>
where
    CurrentBackend: ZombieReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as ZombieReadBackend>::ZombieHandle<'frame>,
}
impl ZombieRef<'_>
where
    CurrentBackend: ZombieReadBackend,
{
    pub fn id(&self) -> ZombieId {
        self.backend.zombie_id(self.handle)
    }
    pub fn hp(&self) -> i32 {
        self.backend.zombie_hp(self.handle)
    }
    pub fn kind(&self) -> ZombieKind {
        read_or_abort(self.backend.zombie_kind(self.handle), "failed to read zombie kind")
    }
    pub fn row(&self) -> i32 {
        self.backend.zombie_row(self.handle)
    }
    pub fn age(&self) -> i32 {
        self.backend.zombie_age(self.handle)
    }
    pub fn is_alive(&self) -> bool {
        self.backend.zombie_is_alive(self.handle)
    }
    pub fn remove(&self)
    where
        CurrentBackend: ZombieRemoveBackend,
    {
        read_or_abort(self.backend.remove_zombie(self.handle), "failed to remove zombie");
    }
    pub fn kill(&self)
    where
        CurrentBackend: ZombieKillBackend,
    {
        read_or_abort(self.backend.kill_zombie(self.handle), "failed to kill zombie");
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: rsvz_backend_api::ZombieRawFactsBackend,
{
    pub fn from_wave(&self) -> i32 {
        self.backend.zombie_from_wave(self.handle)
    }
    pub fn phase_counter(&self) -> i32 {
        self.backend.zombie_phase_counter(self.handle)
    }
    pub fn speed_x(&self) -> f32 {
        self.backend.zombie_speed_x(self.handle)
    }
    pub fn is_blown_away(&self) -> bool {
        self.backend.zombie_is_blown_away(self.handle)
    }
    pub fn chilled_countdown(&self) -> i32 {
        self.backend.zombie_chilled_countdown(self.handle)
    }
    pub fn is_frozen(&self) -> bool {
        self.backend.zombie_frozen_countdown(self.handle) > 0
    }
    pub fn x(&self) -> f32 {
        self.backend.zombie_pos_x(self.handle)
    }
    pub fn phase(&self) -> rsvz_model::ZombiePhase {
        read_or_abort(self.backend.zombie_phase(self.handle), "failed to read zombie phase")
    }
    pub fn animation_progress(&self) -> Option<f32> {
        read_or_abort(
            self.backend.zombie_reanim_anim_time(self.handle),
            "failed to read zombie animation",
        )
    }
    /// Samples motion from this borrowed slot; a dead object has no active motion state.
    pub fn motion_state(&self) -> Option<rsvz_model::ZombieMotionState> {
        read_or_abort(
            crate::logic::zombie_motion::zombie_motion_state_from_handle(self.backend, self.id(), self.handle),
            "failed to read zombie motion",
        )
    }
    /// Predicts the existing stable-motion model, not future phase changes or interactions.
    pub fn stable_x_trace(&self, horizon_frames: u32) -> Result<Vec<f32>, rsvz_model::ZombieMotionCallError> {
        let state = self
            .motion_state()
            .ok_or(rsvz_model::ZombieMotionCallError::ObjectUnavailable)?;
        crate::logic::zombie_motion::predict_stable_zombie_x_trace(&state, horizon_frames)
            .map_err(rsvz_model::ZombieMotionCallError::Motion)
    }
    /// Predicts one coordinate without allocating a trace.
    pub fn stable_x_at(&self, after_frames: u32) -> Result<f32, rsvz_model::ZombieMotionCallError> {
        let state = self
            .motion_state()
            .ok_or(rsvz_model::ZombieMotionCallError::ObjectUnavailable)?;
        crate::logic::zombie_motion::predict_stable_zombie_x_at(&state, after_frames)
            .map_err(rsvz_model::ZombieMotionCallError::Motion)
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: rsvz_backend_api::ZombieVerticalPositionBackend,
{
    pub fn y(&self) -> f32 {
        self.backend.zombie_pos_y(self.handle)
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: rsvz_backend_api::ZombieStateBackend,
{
    pub fn accessory_1_hp(&self) -> i32 {
        self.backend.zombie_accessory_1_hp(self.handle)
    }
    pub fn accessory_2_hp(&self) -> i32 {
        self.backend.zombie_accessory_2_hp(self.handle)
    }
}

impl PlantRef<'_>
where
    CurrentBackend: PlantReadBackend,
{
    pub fn x(&self) -> i32 {
        self.backend.plant_x(self.handle)
    }
    pub fn wake_up_counter(&self) -> i32 {
        self.backend.plant_wake_up_counter(self.handle)
    }
    pub fn state_countdown(&self) -> i32 {
        self.backend.plant_state_countdown(self.handle)
    }
    pub fn effect_countdown(&self) -> i32 {
        self.backend.plant_effect_countdown(self.handle)
    }
    pub fn disappear_countdown(&self) -> i32 {
        self.backend.plant_disappear_countdown(self.handle)
    }
    pub fn on_bungee_state(&self) -> i32 {
        self.backend.plant_on_bungee_state(self.handle)
    }
    pub fn animation_progress(&self) -> Option<f32> {
        read_or_abort(
            self.backend.plant_reanim_circulation(self.handle),
            "plant_reanim_circulation",
        )
    }
}

impl PlantRef<'_>
where
    CurrentBackend: PlantStateBackend,
{
    pub fn y(&self) -> i32 {
        self.backend.plant_y(self.handle)
    }
    pub fn max_hp(&self) -> i32 {
        self.backend.plant_max_hp(self.handle)
    }
    pub fn recently_eaten_countdown(&self) -> i32 {
        self.backend.plant_recently_eaten_countdown(self.handle)
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: ZombieRawFactsBackend,
{
    pub fn altitude(&self) -> f32 {
        self.backend.zombie_altitude(self.handle)
    }
    pub fn is_eating(&self) -> bool {
        self.backend.zombie_is_eating(self.handle)
    }
    pub fn is_mind_controlled(&self) -> bool {
        self.backend.zombie_is_mind_controlled(self.handle)
    }
    pub fn has_object(&self) -> bool {
        self.backend.zombie_has_object(self.handle)
    }
    pub fn has_head(&self) -> bool {
        self.backend.zombie_has_head(self.handle)
    }
    pub fn has_flat_tires(&self) -> bool {
        self.backend.zombie_has_flat_tires(self.handle)
    }
    pub fn is_in_pool(&self) -> bool {
        self.backend.zombie_is_in_pool(self.handle)
    }
    pub fn is_on_high_ground(&self) -> bool {
        self.backend.zombie_is_on_high_ground(self.handle)
    }
    pub fn buttered_countdown(&self) -> i32 {
        self.backend.zombie_buttered_countdown(self.handle)
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: ZombieStateBackend,
{
    pub fn max_hp(&self) -> i32 {
        self.backend.zombie_max_hp(self.handle)
    }
    pub fn accessory_1_max_hp(&self) -> i32 {
        self.backend.zombie_accessory_1_max_hp(self.handle)
    }
    pub fn accessory_2_max_hp(&self) -> i32 {
        self.backend.zombie_accessory_2_max_hp(self.handle)
    }
    pub fn collision(&self) -> ContactRect {
        read_or_abort(self.backend.zombie_collision(self.handle), "zombie_collision")
    }
}

impl ZombieRef<'_>
where
    CurrentBackend: ZombieContactBackend,
{
    pub fn is_dead_or_dying(&self) -> bool {
        self.backend.zombie_is_dead_or_dying(self.handle)
    }
}

impl PlantRef<'_>
where
    CurrentBackend: PlantContactBackend,
{
    pub fn hit_box(&self) -> ContactRect {
        read_or_abort(self.backend.plant_hit_box(self.handle), "plant_hit_box")
    }
    pub fn attack_rect(&self, weapon: PlantWeapon) -> ContactRect {
        read_or_abort(self.backend.plant_attack_rect(self.handle, weapon), "plant_attack_rect")
    }
    pub fn damage_range_flags(&self, weapon: PlantWeapon) -> DamageRangeFlags {
        self.backend.plant_damage_range_flags(self.handle, weapon)
    }
}
impl Frame<'_> {
    /// Visits actual occupancy, including the second cell of a cob cannon.
    pub fn for_each_plant_at_grid(&self, grid: Grid, mut visitor: impl FnMut(PlantRef<'_>))
    where
        CurrentBackend: PlantReadBackend,
    {
        read_or_abort(
            self.backend.for_each_plant_at_grid(grid, |handle| {
                visitor(PlantRef {
                    backend: self.backend,
                    handle,
                });
                Ok(())
            }),
            "for_each_plant_at_grid",
        );
    }
}

/// A borrowed native grid_item; valid only within its shared frame scope.
///
/// ```compile_fail
/// let escaped = rsvz_game::with_frame(|frame| frame.grid_items().next());
/// ```
pub struct GridItemRef<'frame>
where
    CurrentBackend: GridItemReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as GridItemReadBackend>::GridItemHandle<'frame>,
}
impl Frame<'_> {
    pub fn grid_items(&self) -> impl Iterator<Item = GridItemRef<'_>> + '_
    where
        CurrentBackend: GridItemReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.grid_items(), "grid_items").map(move |handle| GridItemRef { backend, handle })
    }
}
impl GridItemRef<'_>
where
    CurrentBackend: GridItemReadBackend,
{
    pub fn id(&self) -> GridItemId {
        self.backend.grid_item_id(self.handle)
    }
    pub fn kind(&self) -> GridItemKind {
        read_or_abort(self.backend.grid_item_kind(self.handle), "grid_item_kind")
    }
    pub fn is_alive(&self) -> bool {
        self.backend.grid_item_is_alive(self.handle)
    }
}

/// ```compile_fail
/// rsvz_game::with_frame(|frame| {
///     let projectile = frame.projectiles().next().unwrap();
///     rsvz_game::tick::on_frame(move |_| { projectile.x(); });
/// });
/// ```
/// A borrowed native projectile; valid only within its shared frame scope.
///
/// ```compile_fail
/// let escaped = rsvz_game::with_frame(|frame| frame.projectiles().next());
/// ```
pub struct ProjectileRef<'frame>
where
    CurrentBackend: ProjectileReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as ProjectileReadBackend>::ProjectileHandle<'frame>,
}
impl Frame<'_> {
    pub fn projectiles(&self) -> impl Iterator<Item = ProjectileRef<'_>> + '_
    where
        CurrentBackend: ProjectileReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.projectiles(), "projectiles").map(move |handle| ProjectileRef { backend, handle })
    }
}
impl ProjectileRef<'_>
where
    CurrentBackend: ProjectileReadBackend,
{
    pub fn id(&self) -> ProjectileId {
        self.backend.projectile_id(self.handle)
    }
    pub fn kind_code(&self) -> i32 {
        self.backend.projectile_kind(self.handle)
    }
    pub fn motion_code(&self) -> i32 {
        self.backend.projectile_motion(self.handle)
    }
    pub fn row(&self) -> i32 {
        self.backend.projectile_row(self.handle)
    }
    pub fn x(&self) -> f32 {
        self.backend.projectile_pos_x(self.handle)
    }
    pub fn y(&self) -> f32 {
        self.backend.projectile_pos_y(self.handle)
    }
    pub fn z(&self) -> f32 {
        self.backend.projectile_pos_z(self.handle)
    }
    pub fn velocity_x(&self) -> f32 {
        self.backend.projectile_vel_x(self.handle)
    }
    pub fn velocity_y(&self) -> f32 {
        self.backend.projectile_vel_y(self.handle)
    }
    pub fn velocity_z(&self) -> f32 {
        self.backend.projectile_vel_z(self.handle)
    }
    pub fn acceleration(&self) -> f32 {
        self.backend.projectile_acceleration(self.handle)
    }
    pub fn collision(&self) -> ContactRect {
        read_or_abort(self.backend.projectile_collision(self.handle), "projectile_collision")
    }
    pub fn damage_flags(&self) -> u32 {
        self.backend.projectile_damage_flags(self.handle)
    }
    pub fn age(&self) -> i32 {
        self.backend.projectile_age(self.handle)
    }
}

/// ```compile_fail
/// rsvz_game::with_frame(|frame| {
///     let seed = frame.seeds().next().unwrap();
///     std::thread::scope(|s| { s.spawn(move || seed.slot()); });
/// });
/// ```
/// A borrowed native seed; valid only within its shared frame scope.
///
/// ```compile_fail
/// let escaped = rsvz_game::with_frame(|frame| frame.seeds().next());
/// ```
pub struct SeedRef<'frame>
where
    CurrentBackend: SeedBankReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as SeedBankReadBackend>::SeedHandle<'frame>,
}
impl Frame<'_> {
    pub fn seeds(&self) -> impl Iterator<Item = SeedRef<'_>> + '_
    where
        CurrentBackend: SeedBankReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.seeds(), "seeds").map(move |handle| SeedRef { backend, handle })
    }
}
impl SeedRef<'_>
where
    CurrentBackend: SeedBankReadBackend,
{
    pub fn slot(&self) -> SeedSlot {
        self.backend.seed_slot(self.handle)
    }
}

/// A borrowed native item; valid only within its shared frame scope.
///
/// ```compile_fail
/// let escaped = rsvz_game::with_frame(|frame| frame.items().next());
/// ```
pub struct ItemRef<'frame>
where
    CurrentBackend: ItemReadBackend,
{
    backend: &'frame CurrentBackend,
    handle: <CurrentBackend as ItemReadBackend>::ItemHandle<'frame>,
}
impl Frame<'_> {
    pub fn items(&self) -> impl Iterator<Item = ItemRef<'_>> + '_
    where
        CurrentBackend: ItemReadBackend,
    {
        let backend = self.backend;
        read_or_abort(backend.items(), "items").map(move |handle| ItemRef { backend, handle })
    }
}
impl ItemRef<'_>
where
    CurrentBackend: ItemReadBackend,
{
    pub fn id(&self) -> ItemId {
        self.backend.item_id(self.handle)
    }
    pub fn kind(&self) -> ItemKind {
        read_or_abort(self.backend.item_kind(self.handle), "item_kind")
    }
    pub fn x(&self) -> f32 {
        self.backend.item_x(self.handle)
    }
    pub fn y(&self) -> f32 {
        self.backend.item_y(self.handle)
    }
    pub fn being_collected(&self) -> bool {
        self.backend.item_being_collected(self.handle)
    }
}

impl GridItemRef<'_>
where
    CurrentBackend: GridItemReadBackend,
{
    pub fn grid(&self) -> Grid {
        Grid {
            row: self.backend.grid_item_row(self.handle),
            col: self.backend.grid_item_col(self.handle),
        }
    }
}
impl GridItemRef<'_>
where
    CurrentBackend: GridItemStateBackend,
{
    pub fn countdown(&self) -> i32 {
        self.backend.grid_item_countdown(self.handle)
    }
}
impl GridItemRef<'_>
where
    CurrentBackend: GridItemEditBackend,
{
    pub fn remove(&self) {
        read_or_abort(self.backend.remove_grid_item(self.handle), "remove_grid_item");
    }
}
impl SeedRef<'_>
where
    CurrentBackend: SeedBankReadBackend,
{
    pub fn selection(&self) -> CardSelection {
        read_or_abort(self.backend.seed_selection(self.handle), "seed_selection").selection()
    }
}
impl SeedRef<'_>
where
    CurrentBackend: SeedCooldownReadBackend,
{
    pub fn cooldown_remaining(&self) -> i32 {
        self.backend.seed_cooldown_remaining(self.handle)
    }
}
impl SeedRef<'_>
where
    CurrentBackend: SeedPacketBackend,
{
    /// Native pickup conditions, including sun and cooldown; does not check a planting grid.
    pub fn can_pick_up(&self) -> bool {
        read_or_abort(self.backend.seed_can_pick_up(self.handle), "seed_can_pick_up")
    }
}
impl ItemRef<'_>
where
    CurrentBackend: ItemClickCollectBackend,
{
    pub fn collect(&self, play_sound: bool) {
        read_or_abort(
            self.backend.click_collect_item(self.handle, play_sound),
            "click_collect_item",
        );
    }
}
