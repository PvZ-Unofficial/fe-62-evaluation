//! Presentation of validated PE observations under advanced pause.
use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::{AdvancedPauseBackend, DeadObjectReclaimBackend};
use rsvz_model::model::pe_replay::{Entity, Frame, Objects};

impl PortableBackend {
    pub fn present_observation_frame(&mut self, frame: Frame<'_>, objects: &mut Objects) -> Result<()> {
        if !self.advanced_pause_active() {
            return Err(PortableBackendError::OperationRejected(
                "PE presentation requires advanced pause",
            ));
        }
        {
            let world = self.world()?;
            for group in 0..4 {
                let mut live = [0u32; 1024];
                for e in frame.entities(group) {
                    live[(e.u(0) & 0xffff) as usize] = e.u(0);
                }
                for (slot, pair) in objects.ids[group].iter_mut().enumerate() {
                    if pair.0 != 0 && pair.0 != live[slot] {
                        match group {
                            0 => {
                                if let Some(p) = world.plants().try_to_get(pair.1) {
                                    p.die()?;
                                }
                            }
                            1 => {
                                if let Some(z) = world.zombies().try_to_get(pair.1) {
                                    z.die_no_loot()?;
                                }
                            }
                            2 => {
                                if let Some(p) = world.projectiles().try_to_get(pair.1) {
                                    unsafe {
                                        (*p.as_ptr()).mDead = true;
                                    }
                                }
                            }
                            _ => {
                                if let Some(g) = world.grid_items().try_to_get(pair.1) {
                                    g.die()?;
                                }
                            }
                        }
                        *pair = (0, 0);
                    }
                }
            }
        }
        self.reclaim_dead_objects()?;
        let world = self.world()?;
        let h = frame.header();
        world.set_sun(h.u(5))?;
        unsafe {
            pvzp_rs::raw::pvzp_rs_replay_clock(world.as_ptr(), h.u(1), h.i(3));
        }
        for row in 0..6 {
            unsafe {
                pvzp_rs::raw::pvzp_rs_replay_ice(
                    world.as_ptr(),
                    row,
                    h.i(41 + row as usize * 2),
                    h.i(42 + row as usize * 2),
                );
            }
        }
        for e in frame.entities(0) {
            let pair = &mut objects.ids[0][(e.u(0) & 0xffff) as usize];
            let p = if pair.0 == e.u(0) {
                world.plants().try_to_get(pair.1)
            } else {
                None
            };
            let p = match p {
                Some(p) => p,
                None => world
                    .new_plant(e.i(3), e.i(2), e.i(1), e.i(8))?
                    .ok_or(PortableBackendError::OperationRejected("replay plant allocation"))?,
            };
            *pair = (e.u(0), world.plants().get_id(p)?);
            // SAFETY: exclusively paused Board; p is validated in this world, no reclamation occurs below.
            unsafe {
                let p = p.as_ptr();
                (*p)._base.mX = e.i(4);
                (*p)._base.mY = e.i(5);
                (*p).mPlantHealth = e.i(6);
                (*p).mState = e.i(7);
                (*p).mIsAsleep = e.u(9) != 0;
                (*p).mDoSpecialCountdown = e.i(10);
                (*p).mStateCountdown = e.i(11);
                (*p).mLaunchCounter = e.i(12);
                (*p).mShootingCounter = e.i(13);
                (*p).mRecentlyEatenCountdown = e.i(14);
                (*p).mTargetX = e.i(15);
                (*p).mTargetY = e.i(16);
                (*p).mSquished = e.u(17) != 0;
                pvzp_rs::raw::pvzp_rs_replay_plant_damage(p);
            }
            if let Some(a) = p.reanimation()? {
                unsafe {
                    animation(a.as_ptr(), e, 18)?;
                }
            }
            p.update_reanim_color()?;
        }
        for e in frame.entities(1) {
            let pair = &mut objects.ids[1][(e.u(0) & 0xffff) as usize];
            let z = if pair.0 == e.u(0) {
                world.zombies().try_to_get(pair.1)
            } else {
                None
            };
            let z = match z {
                Some(z) => z,
                None => world
                    .add_zombie_in_row(e.i(1), e.i(2), e.i(18))?
                    .ok_or(PortableBackendError::OperationRejected("replay zombie allocation"))?,
            };
            *pair = (e.u(0), world.zombies().get_id(z)?);
            unsafe {
                let z = z.as_ptr();
                (*z).mPosX = e.f(3);
                (*z).mPosY = e.f(4);
                (*z)._base.mX = e.i(5);
                (*z)._base.mY = e.i(6);
                (*z).mBodyHealth = e.i(7);
                (*z).mHelmHealth = e.i(8);
                (*z).mShieldHealth = e.i(9);
                (*z).mZombiePhase = e.i(10);
                (*z).mVelX = e.f(12);
                (*z).mIceTrapCounter = e.i(13);
                (*z).mChilledCounter = e.i(14);
                (*z).mButteredCounter = e.i(15);
                (*z).mIsEating = e.u(16) != 0;
                (*z).mZombieAge = e.i(17);
                (*z).mFromWave = e.i(18);
                (*z).mAltitude = e.f(19);
                (*z).mPhaseCounter = e.i(20);
                (*z).mOriginalAnimRate = e.f(21);
                (*z).mMindControlled = e.u(25) != 0;
                (*z).mHasHead = e.u(26) != 0;
                (*z).mBlowingAway = e.u(27) != 0;
                (*z).mHasObject = e.u(35) != 0;
                pvzp_rs::raw::pvzp_rs_replay_gargantuar_damage(z);
            }
            if let Some(a) = z.reanimation()? {
                unsafe {
                    animation(a.as_ptr(), e, 28)?;
                    if e.i(1) == 17 {
                        pvzp_rs::raw::pvzp_rs_replay_digger_facing(
                            a.as_ptr(),
                            e.f(3) - e.i(5) as f32,
                            e.digger_faces_right(),
                        );
                    }
                    (*a.as_ptr()).mLoopCount = e.i(34);
                }
            }
        }
        for e in frame.entities(2) {
            let pair = &mut objects.ids[2][(e.u(0) & 0xffff) as usize];
            let p = if pair.0 == e.u(0) {
                world.projectiles().try_to_get(pair.1)
            } else {
                None
            };
            let p = match p {
                Some(p) => p,
                None => {
                    let raw = unsafe {
                        pvzp_rs::raw::pvzp_rs_replay_projectile(world.as_ptr(), e.i(1), e.i(2), e.i(12), e.i(13))
                    };
                    let ptr = std::ptr::NonNull::new(raw)
                        .ok_or(PortableBackendError::OperationRejected("replay projectile allocation"))?;
                    unsafe { pvzp_rs::Borrowed::from_non_null(ptr) }
                }
            };
            *pair = (e.u(0), world.projectiles().get_id(p)?);
            unsafe {
                let p = p.as_ptr();
                (*p).mPosX = e.f(3);
                (*p).mPosY = e.f(4);
                (*p).mPosZ = e.f(5);
                (*p).mVelX = e.f(6);
                (*p).mVelY = e.f(7);
                (*p).mVelZ = e.f(8);
                (*p).mAccZ = e.f(9);
                (*p).mMotionType = e.i(10);
                (*p).mProjectileAge = e.i(11);
                (*p)._base.mX = e.i(12);
                (*p)._base.mY = e.i(13);
                (*p)._base.mVisible = e.u(14) != 0;
                (*p).mShadowY = e.f(15);
            }
        }
        for e in frame.entities(3) {
            let pair = &mut objects.ids[3][(e.u(0) & 0xffff) as usize];
            let g = if pair.0 == e.u(0) {
                world.grid_items().try_to_get(pair.1)
            } else {
                None
            };
            let g = match g {
                Some(g) => g,
                None => match e.i(1) {
                    1 => world.add_gravestone(e.i(3), e.i(2))?,
                    2 => world.add_crater(e.i(3), e.i(2))?,
                    _ => world.add_ladder(e.i(3), e.i(2))?,
                }
                .ok_or(PortableBackendError::OperationRejected("replay terrain allocation"))?,
            };
            *pair = (e.u(0), world.grid_items().get_id(g)?);
            unsafe {
                (*g.as_ptr()).mGridItemCounter = e.i(4);
            }
        }
        for slot in 0..10 {
            let p = world.seed_storage_at(slot)?;
            let i = 11 + slot as usize * 3;
            unsafe {
                let p = p.as_ptr();
                (*p).mPacketType = h.i(i);
                (*p).mImitaterType = h.i(i + 1);
                let full = pvzp_rs::seed_base_recharge(if h.i(i) == 48 { h.i(i + 1) } else { h.i(i) });
                (*p).mRefreshTime = full;
                (*p).mRefreshCounter = (full - h.i(i + 2)).max(0);
                (*p).mRefreshing = h.i(i + 2) > 0;
                (*p).mActive = h.i(i + 2) == 0;
            }
        }
        Ok(())
    }
}
unsafe fn animation(p: *mut pvzp_rs::raw::pvzp_rs_reanimation, e: Entity<'_>, at: usize) -> Result<()> {
    if e.u(at + 4) == 0 {
        return Ok(());
    }
    let count = unsafe { pvzp_rs::raw::pvzp_rs_replay_frame_count((*p).mReanimationType as i32) };
    if e.i(at + 3) < 0 || e.i(at + 3).checked_add(e.i(at + 4)).is_none_or(|end| end > count) {
        return Err(PortableBackendError::OperationRejected(
            "PE animation outside native definition",
        ));
    }
    // Scalar pose restoration; no game Update or animation event is executed.
    unsafe {
        (*p).mAnimTime = e.f(at);
        (*p).mLastFrameTime = e.f(at + 1);
        (*p).mAnimRate = e.f(at + 2);
        (*p).mFrameStart = e.i(at + 3);
        (*p).mFrameCount = e.i(at + 4);
        (*p).mLoopType = e.i(at + 5);
    }
    Ok(())
}
