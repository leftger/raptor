use crate::config;
use crate::state::{DirectorySceneRoot, InteractionMode, LightcycleSceneRoot, TrailSceneRoot};
use bevy::camera::Projection;
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use std::f32::consts::PI;

/// The flight that carries the app between the explorer and the lightcycle.
///
/// The two modes own different cameras and different scenes, so swapping both
/// on one frame reads as a hard cut. Instead the camera pulls back into a
/// satellite view of the city and then zooms back in on a single road, and the
/// scene change is folded into that move: the outgoing world sinks into the
/// ground on the way up, is replaced while both worlds are flat, and the new
/// one grows back out of the ground as the camera comes down on it. Nothing
/// blinks, and the camera lands exactly on the rig the destination mode was
/// going to use.
pub struct ModeTransitionPlugin;

impl Plugin for ModeTransitionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModeTransition>()
            .add_systems(Startup, setup_rez_wave)
            .add_systems(Update, (advance_transition, update_rez_wave).chain())
            .add_systems(
                PostUpdate,
                // The world rez reads the flight after the camera may have
                // ended it, so the frame the flight lands on is also the frame
                // both worlds are handed back at full height.
                (drive_transition_camera, update_world_rez)
                    .chain()
                    .before(TransformSystems::Propagate),
            );
    }
}

/// Neither fold may run past the end of the leg it belongs to, or a world would
/// still be sinking when the camera it belongs to is already looking at it.
const _: () = assert!(config::MODE_TRANSITION_DEREZ_WINDOW <= config::MODE_TRANSITION_SWAP_AT);
const _: () = assert!(config::MODE_TRANSITION_REZ_WINDOW < 1.0 - config::MODE_TRANSITION_SWAP_AT);

#[derive(Resource, Default)]
pub struct ModeTransition {
    flight: Option<Flight>,
}

struct Flight {
    /// Mode the flight is heading for.
    target: InteractionMode,
    /// Camera pose the flight left from.
    from: Transform,
    /// Satellite view the flight pulls back to and zooms out of.
    apex: Transform,
    /// Camera pose the destination mode's own rig will hold on arrival.
    to: Transform,
    /// Ground point under the satellite view: the stretch of road being zoomed
    /// into, and the center of the rez wave.
    focus: Vec3,
    elapsed: f32,
    /// The world has already been rebuilt for [`Self::target`].
    swapped: bool,
    /// Field of view before the zoom widened it, captured on the first frame
    /// the camera is driven so the kick cannot compound frame over frame.
    base_fov: Option<f32>,
}

impl Flight {
    fn progress(&self) -> f32 {
        (self.elapsed / config::MODE_TRANSITION_DURATION).clamp(0.0, 1.0)
    }

    fn tint(&self) -> Color {
        match self.target {
            InteractionMode::Lightcycle => config::MODE_TRANSITION_LIGHTCYCLE_TINT,
            InteractionMode::Explorer => config::MODE_TRANSITION_EXPLORER_TINT,
        }
    }

    /// Vertical scale for the directory scene and for the arena at this point
    /// in the flight. Whichever one is being left behind sinks, and whichever
    /// one is being arrived at grows.
    fn world_scales(&self) -> (f32, f32) {
        let leaving = derez_scale(self.progress());
        let arriving = rez_scale(self.progress());
        match self.target {
            InteractionMode::Lightcycle => (leaving, arriving),
            InteractionMode::Explorer => (arriving, leaving),
        }
    }
}

impl ModeTransition {
    /// Begins a flight from the live camera pose, out to `apex`, and back in to
    /// the pose `target`'s own camera will hold.
    pub fn start(
        &mut self,
        target: InteractionMode,
        from: Transform,
        apex: Transform,
        to: Transform,
        focus: Vec3,
    ) {
        self.flight = Some(Flight {
            target,
            from,
            apex,
            to,
            focus,
            elapsed: 0.0,
            swapped: false,
            base_fov: None,
        });
    }

    pub fn is_active(&self) -> bool {
        self.flight.is_some()
    }

    /// The mode a running flight is due to swap the world over to, once it has
    /// pulled back far enough and while that swap is still outstanding.
    pub fn pending_swap(&self) -> Option<InteractionMode> {
        self.flight
            .as_ref()
            .filter(|flight| {
                !flight.swapped && flight.progress() >= config::MODE_TRANSITION_SWAP_AT
            })
            .map(|flight| flight.target)
    }

    /// Records that the world now matches [`Self::pending_swap`]'s mode.
    pub fn mark_swapped(&mut self) {
        if let Some(flight) = self.flight.as_mut() {
            flight.swapped = true;
        }
    }
}

/// Run condition for systems that must not fight the flight for the camera.
pub fn transition_inactive(transition: Res<ModeTransition>) -> bool {
    !transition.is_active()
}

/// Run condition for systems that have to stand down while a flight is running.
pub fn transition_active(transition: Res<ModeTransition>) -> bool {
    transition.is_active()
}

/// The satellite view a flight pulls back to: high above `ground` and well off
/// vertical, so the road `along` runs up the screen with a skyline behind it.
pub fn gods_eye_pose(ground: Vec3, along: Vec3) -> Transform {
    let along = along.normalize_or(Vec3::X);
    let position = ground + Vec3::Y * config::MODE_TRANSITION_GODS_EYE_HEIGHT
        - along * config::MODE_TRANSITION_GODS_EYE_BACKOFF;
    // The road itself is handed in as the up vector, which settles the roll of
    // a shot that would otherwise only be constrained by its aim.
    Transform::from_translation(position).looking_at(ground, along)
}

/// Height a world's geometry was built at, kept while the flight scales it into
/// and out of the ground so it can be handed back exactly as it was found.
#[derive(Component)]
struct RezScale(f32);

/// Sheet of light that sits on the ground as the world is swapped and then
/// sweeps up through the new one.
#[derive(Component)]
struct RezWave;

fn setup_rez_wave(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        RezWave,
        Mesh3d(meshes.add(Cuboid::new(
            config::MODE_TRANSITION_REZ_SPAN,
            0.2,
            config::MODE_TRANSITION_REZ_SPAN,
        ))),
        MeshMaterial3d(
            materials.add(StandardMaterial {
                base_color: config::MODE_TRANSITION_LIGHTCYCLE_TINT
                    .with_alpha(config::MODE_TRANSITION_REZ_ALPHA),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        ),
        Transform::IDENTITY,
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

fn advance_transition(time: Res<Time>, mut transition: ResMut<ModeTransition>) {
    if let Some(flight) = transition.flight.as_mut() {
        flight.elapsed += time.delta_secs();
    }
}

#[allow(clippy::type_complexity)]
fn update_rez_wave(
    transition: Res<ModeTransition>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    wave: Single<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<RezWave>,
    >,
) {
    let (mut transform, mut visibility, material) = wave.into_inner();

    let sweep = transition
        .flight
        .as_ref()
        .and_then(|flight| rez_wave_sweep(flight.progress()).map(|sweep| (flight, sweep)));

    let Some((flight, (height, alpha))) = sweep else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    transform.translation = Vec3::new(flight.focus.x, height, flight.focus.z);
    *visibility = Visibility::Visible;
    if let Some(mut wave_material) = materials.get_mut(&material.0) {
        wave_material.base_color = flight.tint().with_alpha(alpha);
    }
}

fn drive_transition_camera(
    mut transition: ResMut<ModeTransition>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    let Some(flight) = transition.flight.as_mut() else {
        return;
    };
    let (mut transform, mut projection) = camera.into_inner();
    let progress = flight.progress();

    *transform = flight_pose(&flight.from, &flight.apex, &flight.to, progress);
    if let Projection::Perspective(perspective) = &mut *projection {
        let base = *flight.base_fov.get_or_insert(perspective.fov);
        perspective.fov = base * fov_scale(progress);
    }

    // The flight lands exactly on `to` with the field of view back at its base,
    // so the destination mode's own camera picks up from here without a pop.
    if progress >= 1.0 && flight.swapped {
        transition.flight = None;
    }
}

/// Sinks the outgoing world into the ground and grows the incoming one back out
/// of it, so the swap happens between two flat worlds instead of two skylines.
#[allow(clippy::type_complexity)]
fn update_world_rez(
    transition: Res<ModeTransition>,
    mut flying: Local<bool>,
    mut commands: Commands,
    mut directory: Query<
        (Entity, &mut Transform, Option<&RezScale>),
        (
            With<DirectorySceneRoot>,
            Without<LightcycleSceneRoot>,
            Without<TrailSceneRoot>,
        ),
    >,
    mut arena: Query<
        (Entity, &mut Transform, Option<&RezScale>),
        (
            Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>,
            Without<DirectorySceneRoot>,
        ),
    >,
) {
    let scales = transition.flight.as_ref().map(Flight::world_scales);
    // Idle costs nothing, but the frame a flight ends still has to hand both
    // worlds back at the height they were found at.
    if scales.is_none() && !*flying {
        return;
    }
    *flying = scales.is_some();

    let (directory_scale, arena_scale) = match scales {
        Some((directory, arena)) => (Some(directory), Some(arena)),
        None => (None, None),
    };
    scale_world(&mut commands, &mut directory, directory_scale);
    scale_world(&mut commands, &mut arena, arena_scale);
}

/// Applies a vertical scale to a world, or hands it back if `scale` is `None`.
fn scale_world<F: QueryFilter>(
    commands: &mut Commands,
    world: &mut Query<(Entity, &mut Transform, Option<&RezScale>), F>,
    scale: Option<f32>,
) {
    for (entity, mut transform, built_at) in world.iter_mut() {
        match (scale, built_at) {
            (Some(scale), Some(built_at)) => transform.scale.y = built_at.0 * scale,
            (Some(scale), None) => {
                let built_at = transform.scale.y;
                commands.entity(entity).insert(RezScale(built_at));
                transform.scale.y = built_at * scale;
            }
            (None, Some(built_at)) => {
                transform.scale.y = built_at.0;
                commands.entity(entity).remove::<RezScale>();
            }
            (None, None) => {}
        }
    }
}

/// Smoothstep: eases away from one rig and onto the next, spending the speed in
/// the middle of each leg.
fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Position within the pull-back, from the starting rig to the satellite view.
fn climb_progress(t: f32) -> f32 {
    (t / config::MODE_TRANSITION_SWAP_AT).clamp(0.0, 1.0)
}

/// Position within the zoom, from the satellite view down onto the road.
fn dive_progress(t: f32) -> f32 {
    ((t - config::MODE_TRANSITION_SWAP_AT) / (1.0 - config::MODE_TRANSITION_SWAP_AT))
        .clamp(0.0, 1.0)
}

/// Samples the camera flight at normalized time `t`.
///
/// `t` of 0 and 1 reproduce `from` and `to` exactly, which is what lets the
/// flight hand the camera back to a mode's own rig without a jump, and the
/// swap point puts it on `apex`.
fn flight_pose(from: &Transform, apex: &Transform, to: &Transform, t: f32) -> Transform {
    if t <= config::MODE_TRANSITION_SWAP_AT {
        leg_pose(
            from,
            apex,
            climb_progress(t),
            config::MODE_TRANSITION_ALTITUDE_BIAS.recip(),
        )
    } else {
        leg_pose(
            apex,
            to,
            dive_progress(t),
            config::MODE_TRANSITION_ALTITUDE_BIAS,
        )
    }
}

/// One leg of the flight, with altitude eased separately from the ground track.
///
/// `altitude_bias` above 1 holds the camera up while it covers ground and drops
/// it late, which is what makes the second leg read as running out over the
/// road and then descending onto it rather than sliding down a ramp. Below 1 it
/// does the reverse, so the pull-back gains its height before it travels.
fn leg_pose(from: &Transform, to: &Transform, u: f32, altitude_bias: f32) -> Transform {
    let ground = ease_in_out(u);
    let altitude = ground.powf(altitude_bias);
    Transform {
        translation: Vec3::new(
            from.translation.x.lerp(to.translation.x, ground),
            from.translation.y.lerp(to.translation.y, altitude),
            from.translation.z.lerp(to.translation.z, ground),
        ),
        rotation: from.rotation.slerp(to.rotation, ground),
        scale: Vec3::ONE,
    }
}

/// The zoom widens the lens on the way in and gives it back by the landing, so
/// the mode it hands over to renders with the field of view it always uses.
fn fov_scale(t: f32) -> f32 {
    let dive = dive_progress(t);
    1.0 + (PI * dive).sin().max(0.0) * config::MODE_TRANSITION_FOV_KICK
}

/// Vertical scale of the world being left behind: full height until the camera
/// is most of the way out, then folded flat by the time the swap arrives.
fn derez_scale(t: f32) -> f32 {
    let window = config::MODE_TRANSITION_DEREZ_WINDOW;
    let u = (t - (config::MODE_TRANSITION_SWAP_AT - window)) / window;
    flat_floor(1.0 - ease_in_out(u))
}

/// Vertical scale of the world being arrived at: flat at the swap, back to full
/// height well before the camera is low enough to be among it.
fn rez_scale(t: f32) -> f32 {
    let u = (t - config::MODE_TRANSITION_SWAP_AT) / config::MODE_TRANSITION_REZ_WINDOW;
    flat_floor(ease_in_out(u))
}

/// Keeps a flattened world off exactly zero, which would hand the renderer a
/// degenerate transform for the frames it spends there.
fn flat_floor(scale: f32) -> f32 {
    scale.max(1.0e-3)
}

/// Height and opacity of the rez wave at `t`, or `None` before the swap.
///
/// The wave lies on the ground for the frame the world is replaced, covering
/// the redraw, then rises with the city growing under it and thins out as the
/// camera closes on it.
fn rez_wave_sweep(t: f32) -> Option<(f32, f32)> {
    let u = dive_progress(t);
    (u > 0.0).then(|| {
        (
            // Started just clear of the ground so it cannot fight the arena
            // floor for the same depth.
            0.15 + u * config::MODE_TRANSITION_REZ_HEIGHT,
            (1.0 - u).powi(2) * config::MODE_TRANSITION_REZ_ALPHA,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ModeTransition, derez_scale, ease_in_out, flight_pose, fov_scale, gods_eye_pose, rez_scale,
        rez_wave_sweep,
    };
    use crate::config;
    use crate::state::InteractionMode;
    use bevy::math::FloatExt;
    use bevy::prelude::{Transform, Vec3};

    const SWAP: f32 = config::MODE_TRANSITION_SWAP_AT;

    /// A run down the +X road, seen from the explorer's default orbit.
    fn flight() -> (Transform, Transform, Transform, Vec3) {
        let focus = Vec3::new(6.0, 0.0, -2.0);
        let from = Transform::from_xyz(18.0, 14.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y);
        let to = Transform::from_xyz(-8.0, 8.0, -2.0).looking_at(focus, Vec3::Y);
        (from, gods_eye_pose(focus, Vec3::X), to, focus)
    }

    /// Compares orientations by where they aim the camera, since a quaternion
    /// and its negation are the same rotation and `slerp` may return either.
    fn aims_like(pose: &Transform, expected: &Transform) -> bool {
        [Vec3::NEG_Z, Vec3::Y]
            .into_iter()
            .all(|axis| (pose.rotation * axis).abs_diff_eq(expected.rotation * axis, 1.0e-5))
    }

    #[test]
    fn the_satellite_view_looks_down_the_road_from_above_it() {
        let ground = Vec3::new(6.0, 0.0, -2.0);
        let apex = gods_eye_pose(ground, Vec3::X);

        assert!(apex.translation.y > config::MODE_TRANSITION_GODS_EYE_HEIGHT - 1.0e-3);
        let view = apex.rotation * Vec3::NEG_Z;
        assert!(
            view.abs_diff_eq((ground - apex.translation).normalize(), 1.0e-5),
            "the shot has to be aimed at the road it is over"
        );
        // Steeply down, but well off vertical, so a skyline stays in frame.
        assert!((-0.98..-0.6).contains(&view.y), "got {view}");
        // The road leads toward the top of the frame, so the zoom is pointed
        // into the run ahead of the cycle.
        assert!((apex.rotation * Vec3::Y).dot(Vec3::X) > 0.5);
    }

    #[test]
    fn a_degenerate_road_still_produces_a_usable_shot() {
        let apex = gods_eye_pose(Vec3::ZERO, Vec3::ZERO);
        assert!(apex.translation.is_finite());
        assert!(apex.rotation.is_finite());
    }

    #[test]
    fn the_flight_hits_both_rigs_and_the_satellite_view_exactly() {
        let (from, apex, to, _) = flight();

        for (t, expected) in [(0.0, from), (SWAP, apex), (1.0, to)] {
            let pose = flight_pose(&from, &apex, &to, t);
            assert!(
                pose.translation.abs_diff_eq(expected.translation, 1.0e-4),
                "at {t} the flight sat at {} not {}",
                pose.translation,
                expected.translation
            );
            assert!(aims_like(&pose, &expected));
        }
    }

    #[test]
    fn the_pull_back_gains_height_early_and_the_zoom_gives_it_back_late() {
        let (from, apex, to, _) = flight();

        let climbing = flight_pose(&from, &apex, &to, SWAP * 0.5);
        let straight_climb = from
            .translation
            .y
            .lerp(apex.translation.y, ease_in_out(0.5));
        assert!(
            climbing.translation.y > straight_climb,
            "the camera has to be up over the city before it travels"
        );

        let diving = flight_pose(&from, &apex, &to, SWAP + (1.0 - SWAP) * 0.5);
        let straight_dive = apex.translation.y.lerp(to.translation.y, ease_in_out(0.5));
        assert!(
            diving.translation.y > straight_dive,
            "the camera has to hang above the road before dropping onto it"
        );
    }

    #[test]
    fn the_zoom_runs_the_camera_down_onto_the_road() {
        let (from, apex, to, focus) = flight();

        let mut previous = flight_pose(&from, &apex, &to, SWAP);
        for step in 1..=12 {
            let t = SWAP + (1.0 - SWAP) * (step as f32 / 12.0);
            let pose = flight_pose(&from, &apex, &to, t);
            assert!(pose.translation.y <= previous.translation.y + 1.0e-4);
            assert!(
                pose.translation.distance(focus) <= previous.translation.distance(focus) + 1.0e-4
            );
            previous = pose;
        }
    }

    /// The swap is only invisible if neither world has any height left at the
    /// moment it happens, and both are back to full height by the landing.
    #[test]
    fn both_worlds_are_flat_at_the_swap_and_whole_at_both_ends() {
        assert_eq!(derez_scale(0.0), 1.0);
        assert!(derez_scale(SWAP) < 0.01);
        assert!(rez_scale(SWAP) < 0.01);
        assert_eq!(rez_scale(1.0), 1.0);

        // Neither world may be caught mid-fold by the camera it belongs to.
        assert_eq!(
            derez_scale(SWAP - config::MODE_TRANSITION_DEREZ_WINDOW),
            1.0,
            "the outgoing world stands until the camera has pulled back"
        );
        assert_eq!(
            rez_scale(SWAP + config::MODE_TRANSITION_REZ_WINDOW),
            1.0,
            "the incoming world is whole before the camera is down among it"
        );
    }

    #[test]
    fn a_world_folds_and_unfolds_without_reversing() {
        let mut previous = 1.0;
        for step in 0..=40 {
            let scale = derez_scale(SWAP * (step as f32 / 40.0));
            assert!(scale <= previous + 1.0e-6, "the fold has to be monotonic");
            previous = scale;
        }

        let mut previous = 0.0;
        for step in 0..=40 {
            let scale = rez_scale(SWAP + (1.0 - SWAP) * (step as f32 / 40.0));
            assert!(scale >= previous - 1.0e-6, "the unfold has to be monotonic");
            previous = scale;
        }
    }

    #[test]
    fn the_lens_is_back_to_normal_at_both_ends() {
        assert_eq!(fov_scale(0.0), 1.0);
        assert_eq!(fov_scale(1.0), 1.0);
        assert!(fov_scale(SWAP + 0.2) > 1.0, "the zoom widens the lens");
    }

    #[test]
    fn the_rez_wave_carries_the_redraw_then_climbs_out_of_the_way() {
        assert_eq!(rez_wave_sweep(0.0), None);
        assert_eq!(rez_wave_sweep(SWAP), None);

        let (start, start_alpha) = rez_wave_sweep(SWAP + 1.0e-4).expect("wave runs from the swap");
        assert!(start < 0.5, "the wave has to start on the ground");
        assert!(start_alpha > config::MODE_TRANSITION_REZ_ALPHA * 0.9);

        let (end, end_alpha) = rez_wave_sweep(1.0).expect("wave runs to the landing");
        assert!(end > start);
        assert_eq!(end_alpha, 0.0);
    }

    #[test]
    fn the_swap_is_offered_once_and_only_at_the_top_of_the_climb() {
        let (from, apex, to, focus) = flight();
        let mut transition = ModeTransition::default();
        transition.start(InteractionMode::Lightcycle, from, apex, to, focus);

        assert_eq!(transition.pending_swap(), None);

        let flight = transition.flight.as_mut().expect("flight is running");
        flight.elapsed = config::MODE_TRANSITION_DURATION * SWAP;
        assert_eq!(transition.pending_swap(), Some(InteractionMode::Lightcycle));

        transition.mark_swapped();
        assert_eq!(transition.pending_swap(), None);
        assert!(transition.is_active());
    }

    /// Each world is scaled by the flight it is leaving or arriving in, and
    /// never by the other one's curve.
    #[test]
    fn the_world_being_left_sinks_and_the_world_arrived_at_grows() {
        let (from, apex, to, focus) = flight();
        let mut transition = ModeTransition::default();

        transition.start(InteractionMode::Lightcycle, from, apex, to, focus);
        let flight = transition.flight.as_mut().expect("flight is running");
        flight.elapsed = config::MODE_TRANSITION_DURATION * (SWAP - 0.05);
        let (directory, arena) = flight.world_scales();
        assert!(directory < 1.0 && arena < 0.01, "the directory sinks");

        flight.elapsed = config::MODE_TRANSITION_DURATION;
        let (directory, arena) = flight.world_scales();
        assert!(directory < 0.01 && arena == 1.0, "the arena is whole");

        transition.start(InteractionMode::Explorer, from, apex, to, focus);
        let flight = transition.flight.as_mut().expect("flight is running");
        flight.elapsed = config::MODE_TRANSITION_DURATION;
        let (directory, arena) = flight.world_scales();
        assert!(directory == 1.0 && arena < 0.01, "the directory is whole");
    }
}
