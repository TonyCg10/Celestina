//! The mirror's window as a control: `mpv` shows the phone and a script it
//! loads reports the pointer, the wheel and the keys on its log; this side
//! turns those lines into the wire's touches, gestures and keys, scaled
//! from the window to the phone's pixels.

use std::io::{BufRead, BufReader};
use std::process::ChildStdout;
use std::time::Duration;

use magnetita_proto::capability;
use magnetita_proto::mirror::{GlobalAction, MirrorGlobal, MirrorKey, MirrorTouch, TouchAction};
use magnetita_proto::Envelope;

/// The keys the window forwards: `mpv`'s name and Android's key code. A
/// key not here stays with the window.
const KEYS: &[(&str, u16)] = &[
    ("SPACE", 62),
    ("ENTER", 66),
    ("BS", 67),
    ("TAB", 61),
    ("ESC", 111),
    ("DEL", 112),
    ("UP", 19),
    ("DOWN", 20),
    ("LEFT", 21),
    ("RIGHT", 22),
    ("HOME", 122),
    ("END", 123),
    ("PGUP", 92),
    ("PGDWN", 93),
    ("VOLUME_UP", 24),
    ("VOLUME_DOWN", 25),
    ("MUTE", 164),
    ("PLAY", 126),
    ("PAUSE", 127),
    ("PLAYPAUSE", 85),
    ("NEXT", 87),
    ("PREV", 88),
    ("a", 29),
    ("b", 30),
    ("c", 31),
    ("d", 32),
    ("e", 33),
    ("f", 34),
    ("g", 35),
    ("h", 36),
    ("i", 37),
    ("j", 38),
    ("k", 39),
    ("l", 40),
    ("m", 41),
    ("n", 42),
    ("o", 43),
    ("p", 44),
    ("q", 45),
    ("r", 46),
    ("s", 47),
    ("t", 48),
    ("u", 49),
    ("v", 50),
    ("w", 51),
    ("x", 52),
    ("y", 53),
    ("z", 54),
    ("0", 7),
    ("1", 8),
    ("2", 9),
    ("3", 10),
    ("4", 11),
    ("5", 12),
    ("6", 13),
    ("7", 14),
    ("8", 15),
    ("9", 16),
    (",", 55),
    (".", 56),
    ("-", 69),
    ("=", 70),
    ("/", 76),
    ("'", 75),
    (";", 74),
    ("[", 71),
    ("]", 72),
    ("\\", 73),
];

/// The script `mpv` runs: every report is one line `T <D|M|U> x y vx vy vw
/// vh` (the pointer and the video's rectangle inside the window, which a
/// tiling compositor letterboxes), `W <-1|1> x y vx vy vw vh`, `K <name>
/// <1|0>` or `G <0|1|2>` at info level under the `touch` prefix, which is
/// the only thing this side lets `mpv` print. A pointer that leaves the
/// window while pressed lifts, so no finger stays down on the phone.
pub(crate) fn script() -> String {
    let mut lua = String::from(
        r#"local down = false
local function video()
  local d = mp.get_property_native("osd-dimensions")
  if d == nil then return 0, 0, 0, 0 end
  local w = (d.w or 0) - (d.ml or 0) - (d.mr or 0)
  local h = (d.h or 0) - (d.mt or 0) - (d.mb or 0)
  return d.ml or 0, d.mt or 0, w, h
end
local function report(kind, what, x, y)
  local vx, vy, vw, vh = video()
  mp.msg.info(string.format("%s %s %d %d %d %d %d %d", kind, what, x, y, vx, vy, vw, vh))
end
local function pos()
  local p = mp.get_property_native("mouse-pos")
  if p == nil then return 0, 0 end
  return p.x or 0, p.y or 0
end
mp.add_forced_key_binding("MBTN_LEFT", "magnetita_touch", function(e)
  local x, y = pos()
  if e.event == "down" then down = true; report("T", "D", x, y)
  elseif e.event == "up" then down = false; report("T", "U", x, y) end
end, {complex = true})
mp.observe_property("mouse-pos", "native", function(_, p)
  if not down or p == nil then return end
  if p.hover == false then
    down = false
    report("T", "U", p.x or 0, p.y or 0)
  else
    report("T", "M", p.x or 0, p.y or 0)
  end
end)
mp.add_forced_key_binding("MBTN_RIGHT", "magnetita_back", function() mp.msg.info("G 0") end)
mp.add_forced_key_binding("MBTN_MID", "magnetita_home", function() mp.msg.info("G 1") end)
mp.add_forced_key_binding("MBTN_BACK", "magnetita_back2", function() mp.msg.info("G 0") end)
mp.add_forced_key_binding("MBTN_FORWARD", "magnetita_recents", function() mp.msg.info("G 2") end)
mp.add_forced_key_binding("WHEEL_UP", "magnetita_wheel_up", function()
  local x, y = pos(); report("W", "-1", x, y)
end)
mp.add_forced_key_binding("WHEEL_DOWN", "magnetita_wheel_down", function()
  local x, y = pos(); report("W", "1", x, y)
end)
mp.add_forced_key_binding("F1", "magnetita_f1", function() mp.msg.info("G 0") end)
mp.add_forced_key_binding("F2", "magnetita_f2", function() mp.msg.info("G 1") end)
mp.add_forced_key_binding("F3", "magnetita_f3", function() mp.msg.info("G 2") end)
local function key(name)
  mp.add_forced_key_binding(name, "magnetita_key_" .. name, function(e)
    if e.event == "down" or e.event == "repeat" then mp.msg.info("K " .. name .. " 1")
    elseif e.event == "up" then mp.msg.info("K " .. name .. " 0") end
  end, {complex = true})
end
"#,
    );
    for (name, _) in KEYS {
        let quoted = name.replace('\\', "\\\\").replace('\'', "\\'");
        lua.push_str(&format!("key('{quoted}')\n"));
    }
    lua
}

/// The window's reports as the wire's messages, scaled to the phone.
pub(crate) struct Translator {
    width: u16,
    height: u16,
}

impl Translator {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// One window coordinate onto the phone, through the video's rectangle
    /// inside the window; a point in the letterbox clamps to the edge.
    fn scale(&self, x: i64, y: i64, rect: (i64, i64, i64, i64)) -> (u16, u16) {
        let (vx, vy, vw, vh) = rect;
        if vw <= 0 || vh <= 0 {
            return (0, 0);
        }
        let px = ((x - vx).max(0) * i64::from(self.width) / vw).min(i64::from(self.width) - 1);
        let py = ((y - vy).max(0) * i64::from(self.height) / vh).min(i64::from(self.height) - 1);
        (px.max(0) as u16, py.max(0) as u16)
    }

    /// The messages one report line stands for; a wheel tick is a short
    /// swipe, so it carries several with their pacing.
    pub(crate) fn translate(&self, line: &str) -> Vec<(Envelope, Duration)> {
        let line = line.trim();
        let line = line.strip_prefix("[touch]").unwrap_or(line).trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| parts.get(i).and_then(|s| s.parse::<i64>().ok());
        match parts.first().copied() {
            Some("T") if parts.len() >= 8 => {
                let action = match parts[1] {
                    "D" => TouchAction::Down,
                    "M" => TouchAction::Move,
                    "U" => TouchAction::Up,
                    _ => return Vec::new(),
                };
                let (Some(x), Some(y), Some(vx), Some(vy), Some(vw), Some(vh)) =
                    (num(2), num(3), num(4), num(5), num(6), num(7))
                else {
                    return Vec::new();
                };
                let (px, py) = self.scale(x, y, (vx, vy, vw, vh));
                vec![(touch(action, px, py), Duration::ZERO)]
            }
            Some("W") if parts.len() >= 8 => {
                let (Some(dir), Some(x), Some(y), Some(vx), Some(vy), Some(vw), Some(vh)) =
                    (num(1), num(2), num(3), num(4), num(5), num(6), num(7))
                else {
                    return Vec::new();
                };
                let (px, py) = self.scale(x, y, (vx, vy, vw, vh));
                // A wheel tick down scrolls the content up: the finger moves
                // up a fifth of the screen in six steps.
                let travel = i64::from(self.height) / 5;
                let step = Duration::from_millis(12);
                let mut out = vec![(touch(TouchAction::Down, px, py), step)];
                for i in 1..=6 {
                    let dy = -dir * travel * i / 6;
                    let y = (i64::from(py) + dy).clamp(0, i64::from(self.height) - 1) as u16;
                    out.push((touch(TouchAction::Move, px, y), step));
                }
                let end =
                    (i64::from(py) - dir * travel).clamp(0, i64::from(self.height) - 1) as u16;
                out.push((touch(TouchAction::Up, px, end), Duration::ZERO));
                out
            }
            Some("K") if parts.len() >= 3 => {
                let Some(&(_, keycode)) = KEYS.iter().find(|(name, _)| *name == parts[1]) else {
                    return Vec::new();
                };
                let pressed = parts[2] == "1";
                vec![(
                    Envelope {
                        capability: capability::MIRROR,
                        kind: MirrorKey::KIND,
                        id: 0,
                        body: MirrorKey { keycode, pressed }.encode(),
                    },
                    Duration::ZERO,
                )]
            }
            Some("G") if parts.len() >= 2 => {
                let action = match parts[1] {
                    "0" => GlobalAction::Back,
                    "1" => GlobalAction::Home,
                    "2" => GlobalAction::Recents,
                    _ => return Vec::new(),
                };
                vec![(
                    Envelope {
                        capability: capability::MIRROR,
                        kind: MirrorGlobal::KIND,
                        id: 0,
                        body: MirrorGlobal { action }.encode(),
                    },
                    Duration::ZERO,
                )]
            }
            _ => Vec::new(),
        }
    }
}

fn touch(action: TouchAction, x: u16, y: u16) -> Envelope {
    Envelope {
        capability: capability::MIRROR,
        kind: MirrorTouch::KIND,
        id: 0,
        body: MirrorTouch {
            action,
            x,
            y,
            pointer: 0,
        }
        .encode(),
    }
}

/// The least time between two moves sent while a finger is down: the
/// pointer reports far faster than the phone can play a stroke segment,
/// and a queue of stale points is a finger that lands late and elsewhere.
const MOVE_INTERVAL: Duration = Duration::from_millis(8);

/// Reads `mpv`'s log until it closes, handing every translated message to
/// `deliver` with the pacing a gesture needs. Moves closer together than
/// [`MOVE_INTERVAL`] fold into the latest one, sent before the next event.
pub(crate) fn pump(stdout: ChildStdout, translator: Translator, deliver: impl Fn(Envelope)) {
    let reader = BufReader::new(stdout);
    let mut pending_move: Option<Envelope> = None;
    let mut last_move = std::time::Instant::now() - MOVE_INTERVAL;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        for (env, pause) in translator.translate(&line) {
            let is_move = env.kind == MirrorTouch::KIND
                && MirrorTouch::decode(&env.body).is_ok_and(|t| t.action == TouchAction::Move);
            if is_move && pause.is_zero() {
                if last_move.elapsed() < MOVE_INTERVAL {
                    pending_move = Some(env);
                    continue;
                }
                pending_move = None;
                last_move = std::time::Instant::now();
                deliver(env);
                continue;
            }
            if let Some(held) = pending_move.take() {
                deliver(held);
            }
            deliver(env);
            if !pause.is_zero() {
                std::thread::sleep(pause);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_points_scale_to_the_phone_and_keys_map() {
        let t = Translator::new(1080, 2340);
        // A tiled window: the video sits 100 px in from the left.
        let out = t.translate("[touch] T D 370 585 100 0 540 1170");
        assert_eq!(out.len(), 1);
        let touch = MirrorTouch::decode(&out[0].0.body).unwrap();
        assert_eq!(
            (touch.action, touch.x, touch.y),
            (TouchAction::Down, 540, 1170)
        );
        let key = t.translate("K ENTER 1");
        assert_eq!(MirrorKey::decode(&key[0].0.body).unwrap().keycode, 66);
        let global = t.translate("G 2");
        assert_eq!(
            MirrorGlobal::decode(&global[0].0.body).unwrap().action,
            GlobalAction::Recents
        );
        assert!(t.translate("K F9 1").is_empty());
        assert!(t.translate("[cplayer] something").is_empty());
    }

    #[test]
    fn a_wheel_tick_is_a_swipe_that_stays_on_screen() {
        let t = Translator::new(1080, 2340);
        let out = t.translate("W 1 270 100 0 0 540 1170");
        assert_eq!(out.len(), 8);
        let first = MirrorTouch::decode(&out[0].0.body).unwrap();
        let last = MirrorTouch::decode(&out[7].0.body).unwrap();
        assert_eq!(first.action, TouchAction::Down);
        assert_eq!(last.action, TouchAction::Up);
        assert!(last.y < first.y, "scrolling down moves the finger up");
        let up = t.translate("W -1 270 2 0 0 540 1170");
        let end = MirrorTouch::decode(&up[7].0.body).unwrap();
        assert!(end.y > 400);
    }

    #[test]
    fn the_script_binds_every_key_it_reports() {
        let lua = script();
        for (name, _) in KEYS {
            assert!(
                lua.contains(&format!(
                    "key('{}')",
                    name.replace('\\', "\\\\").replace('\'', "\\'")
                )),
                "{name}"
            );
        }
        assert!(lua.contains("MBTN_LEFT"));
    }
}
