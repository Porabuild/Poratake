//! Minimal SVG path-data parser covering everything lucide icons emit
//! (M/L/H/V/C/S/Q/T/A/Z in absolute and relative form), producing absolute
//! segments in the icon's 24x24 coordinate space.

#[derive(Clone, Copy, Debug)]
pub enum PathCommand {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    CubicTo {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x: f32,
        y: f32,
    },
    QuadTo {
        x1: f32,
        y1: f32,
        x: f32,
        y: f32,
    },
    ArcTo {
        rx: f32,
        ry: f32,
        rotation: f32,
        large_arc: bool,
        sweep: bool,
        x: f32,
        y: f32,
    },
    Close,
}

#[derive(Default)]
struct Pen {
    x: f32,
    y: f32,
    start_x: f32,
    start_y: f32,
    last_cubic_x: f32,
    last_cubic_y: f32,
    last_quad_x: f32,
    last_quad_y: f32,
    had_cubic: bool,
    had_quad: bool,
}

struct Tokens<'a> {
    rest: &'a str,
}

impl<'a> Tokens<'a> {
    fn new(data: &'a str) -> Self {
        Self { rest: data }
    }

    fn skip_separators(&mut self) {
        self.rest = self
            .rest
            .trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    }

    /// Peeks whether the next token is a command letter.
    fn next_is_command(&mut self) -> bool {
        self.skip_separators();
        self.rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    }

    fn command(&mut self) -> Option<char> {
        self.skip_separators();
        let mut chars = self.rest.chars();
        let c = chars.next()?;
        if c.is_ascii_alphabetic() {
            self.rest = chars.as_str();
            Some(c)
        } else {
            None
        }
    }

    /// Reads one SVG number; a sign or extra dot starts a new number ("4-4"
    /// parses as two numbers).
    fn number(&mut self) -> Option<f32> {
        self.skip_separators();
        let mut end = 0;
        let bytes = self.rest.as_bytes();
        let mut seen_dot = false;
        while end < bytes.len() {
            let b = bytes[end];
            match b {
                b'0'..=b'9' => end += 1,
                b'.' => {
                    // A dot may start a number (".486"), follow a sign ("-​.9")
                    // or refine one ("1.5"); a second dot ends this token so
                    // "4.5.5" splits cleanly.
                    if seen_dot {
                        break;
                    }
                    seen_dot = true;
                    end += 1;
                }
                b'-' | b'+' => {
                    if end == 0 {
                        end += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        if end == 0 {
            return None;
        }
        let text = &self.rest[..end];
        self.rest = &self.rest[end..];
        text.parse::<f32>().ok()
    }

    fn flag(&mut self) -> Option<bool> {
        self.number().map(|v| v != 0.0)
    }
}

/// Parses path data into absolute commands.
pub fn parse_path(data: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut tokens = Tokens::new(data);
    let mut pen = Pen::default();

    'outer: while let Some(raw_letter) = tokens.command() {
        let relative = raw_letter.is_lowercase();

        // After a moveto, additional coordinate pairs are implicit linetos.
        let mut moveto_continuation = false;

        loop {
            let letter = raw_letter.to_ascii_uppercase();

            // Z takes no parameters and is usually followed directly by
            // another command letter, so it must be handled before the
            // implicit-continuation guard below.
            if letter == 'Z' {
                commands.push(PathCommand::Close);
                pen.x = pen.start_x;
                pen.y = pen.start_y;
                continue 'outer;
            }

            if tokens.next_is_command() {
                continue 'outer;
            }

            match letter {
                'M' | 'L' => {
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let is_move = (raw_letter == 'M' || raw_letter == 'm') && !moveto_continuation;
                    if is_move {
                        commands.push(PathCommand::MoveTo { x, y });
                        pen.start_x = x;
                        pen.start_y = y;
                        moveto_continuation = true;
                    } else {
                        commands.push(PathCommand::LineTo { x, y });
                    }
                    pen.x = x;
                    pen.y = y;
                    continue;
                }
                'H' => {
                    let Some(dx) = tokens.number() else {
                        break 'outer;
                    };
                    let x = if relative { pen.x + dx } else { dx };
                    commands.push(PathCommand::LineTo { x, y: pen.y });
                    pen.x = x;
                }
                'V' => {
                    let Some(dy) = tokens.number() else {
                        break 'outer;
                    };
                    let y = if relative { pen.y + dy } else { dy };
                    commands.push(PathCommand::LineTo { x: pen.x, y });
                    pen.y = y;
                }
                'C' => {
                    let Some((x1, y1)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let Some((x2, y2)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    commands.push(PathCommand::CubicTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    });
                    pen.last_cubic_x = x2;
                    pen.last_cubic_y = y2;
                    pen.had_cubic = true;
                    pen.had_quad = false;
                    pen.x = x;
                    pen.y = y;
                }
                'S' => {
                    let Some((x2, y2)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let (x1, y1) = if pen.had_cubic {
                        (
                            2.0 * pen.x - pen.last_cubic_x,
                            2.0 * pen.y - pen.last_cubic_y,
                        )
                    } else {
                        (pen.x, pen.y)
                    };
                    commands.push(PathCommand::CubicTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    });
                    pen.last_cubic_x = x2;
                    pen.last_cubic_y = y2;
                    pen.had_cubic = true;
                    pen.had_quad = false;
                    pen.x = x;
                    pen.y = y;
                }
                'Q' => {
                    let Some((x1, y1)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    commands.push(PathCommand::QuadTo { x1, y1, x, y });
                    pen.last_quad_x = x1;
                    pen.last_quad_y = y1;
                    pen.had_quad = true;
                    pen.had_cubic = false;
                    pen.x = x;
                    pen.y = y;
                }
                'T' => {
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    let (x1, y1) = if pen.had_quad {
                        (2.0 * pen.x - pen.last_quad_x, 2.0 * pen.y - pen.last_quad_y)
                    } else {
                        (pen.x, pen.y)
                    };
                    commands.push(PathCommand::QuadTo { x1, y1, x, y });
                    pen.last_quad_x = x1;
                    pen.last_quad_y = y1;
                    pen.had_quad = true;
                    pen.had_cubic = false;
                    pen.x = x;
                    pen.y = y;
                }
                'A' => {
                    let (Some(rx), Some(ry), Some(rotation), Some(large_arc), Some(sweep)) = (
                        tokens.number(),
                        tokens.number(),
                        tokens.number(),
                        tokens.flag(),
                        tokens.flag(),
                    ) else {
                        break 'outer;
                    };
                    let Some((x, y)) = pair(&mut tokens, relative, &pen) else {
                        break 'outer;
                    };
                    commands.push(PathCommand::ArcTo {
                        rx,
                        ry,
                        rotation,
                        large_arc,
                        sweep,
                        x,
                        y,
                    });
                    pen.x = x;
                    pen.y = y;
                }
                _ => break 'outer,
            }
        }
    }

    commands
}

fn pair(tokens: &mut Tokens, relative: bool, pen: &Pen) -> Option<(f32, f32)> {
    let dx = tokens.number()?;
    let dy = tokens.number()?;
    if relative {
        Some((pen.x + dx, pen.y + dy))
    } else {
        Some((dx, dy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_icon_two_subpaths() {
        let cmds = parse_path("M 10 8 H 20 A 2 2 0 0 1 22 10 V 20 A 2 2 0 0 1 20 22 H 10 A 2 2 0 0 1 8 20 V 10 A 2 2 0 0 1 10 8 Z M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2");
        for (index, command) in cmds.iter().enumerate() {
            println!("{index}: {command:?}");
        }
        // First subpath closes; the second starts with a fresh moveto.
        let close_index = cmds
            .iter()
            .position(|command| matches!(command, PathCommand::Close))
            .expect("first subpath closes");
        assert!(matches!(
            cmds.get(close_index + 1),
            Some(PathCommand::MoveTo { x: 4.0, y: 16.0 })
        ));
        assert_eq!(cmds.len(), close_index + 7);
    }

    #[test]
    fn camera_body() {
        let cmds = parse_path("M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z");
        for (index, command) in cmds.iter().enumerate() {
            println!("{index}: {command:?}");
        }
        assert!(cmds.len() > 10);
    }

    #[test]
    fn x_icon() {
        let cmds = parse_path("M18 6 6 18 m6 6 12 12");
        for command in &cmds {
            println!("{command:?}");
        }
        assert!(matches!(cmds[0], PathCommand::MoveTo { x: 18.0, y: 6.0 }));
        // Implicit pairs after moveto become linetos, not more movetos.
        assert!(matches!(cmds[1], PathCommand::LineTo { x: 6.0, y: 18.0 }));
        // Relative moveto from the current pen position (6,18).
        assert!(matches!(cmds[2], PathCommand::MoveTo { x: 12.0, y: 24.0 }));
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn check_icon() {
        let cmds = parse_path("M20 6 9 17l-5-5");
        println!("{cmds:?}");
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn circle_arc() {
        let cmds = parse_path("M 2 12 A 10 10 0 1 1 22 12 A 10 10 0 1 1 2 12");
        println!("{cmds:?}");
        assert_eq!(cmds.len(), 3);
        match cmds[1] {
            PathCommand::ArcTo {
                rx,
                ry,
                large_arc,
                sweep,
                x,
                y,
                ..
            } => {
                assert_eq!((rx, ry), (10.0, 10.0));
                assert!(large_arc);
                assert!(sweep);
                assert_eq!((x, y), (22.0, 12.0));
            }
            other => panic!("expected arc, got {other:?}"),
        }
    }
}
