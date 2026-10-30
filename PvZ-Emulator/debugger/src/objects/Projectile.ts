export type ProjectileType =
    "none" |
    "pea" |
    "snow_pea" |
    "cabbage" |
    "melon" |
    "puff" |
    "wintermelon" |
    "fire_pea" |
    "star" |
    "cactus" |
    "basketball" |
    "kernel" |
    "cob_cannon" |
    "butter";

export type ProjectileMotionType =
    "straight" |
    "parabola" |
    "switch_way" |
    "puff" |
    "left_straight" |
    "starfruit" |
    "cattail";

export interface Projectile {
    id: number,

    type: ProjectileType,
    motion_type: ProjectileMotionType,

    int_x: number,
    int_y: number,

    row: number,

    y: number,
    x: number,
    shadow_y: number,

    dx: number,

    dy1: number,
    dy2: number,

    ddy: number,
    dddy: number,

    attack_box: {
        x: number,
        y: number,
        width: number,
        height: number
    },

    flags: {
        ground: boolean,
        flying_balloon: boolean,
        lurking_snorkel: boolean,
        dog: boolean,
        animating_zombies: boolean,
        dying_zombies: boolean,
        digging_digger: boolean,
        hypno_zombies: boolean
    },

    time_since_created: number,
    countdown: number,

    last_torchwood_col: number | null,

    cannon_x: number,
    cannon_row: number,

    target: number | null,

    is_visible: boolean,
    is_disappearaed: boolean
}
