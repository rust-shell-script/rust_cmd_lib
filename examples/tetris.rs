#![allow(non_upper_case_globals)]
use std::io::Read;
use std::{thread, time};
use std::collections::VecDeque;
use cmd_lib::{
    CmdResult,
    proc_var,
    proc_var_get,
    proc_var_set,
    run_cmd,
    run_fun,
};

// Tetris game converted from bash version:
// https://github.com/kt97679/tetris

// Original comments:
// Tetris game written in pure bash
// I tried to mimic as close as possible original tetris game
// which was implemented on old soviet DVK computers (PDP-11 clones)
//
// Videos of this tetris can be found here:
//
// http://www.youtube.com/watch?v=O0gAgQQHFcQ
// http://www.youtube.com/watch?v=iIQc1F3UuV4
//
// This script was created on ubuntu 13.04 x64 and bash 4.2.45(1)-release.
// It was not tested on other unix like operating systems.
//
// Enjoy :-)!
//
// Author: Kirill Timofeev <kt97679@gmail.com>
//
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See http://www.wtfpl.net/ for more details.

proc_var!(DELAY, f64, 1.0);     // initial delay between piece movements
const DELAY_FACTOR: f64 = 0.8;  // this value controld delay decrease for each level up

// color codes
const RED:      i32 = 1;
const GREEN:    i32 = 2;
const YELLOW:   i32 = 3;
const BLUE:     i32 = 4;
const FUCHSIA:  i32 = 5;
const CYAN:     i32 = 6;
const WHITE:    i32 = 7;

// Location and size of playfield, color of border
const PLAYFIELD_W:  i32 = 10;
const PLAYFIELD_H:  i32 = 20;
const PLAYFIELD_X:  i32 = 30;
const PLAYFIELD_Y:  i32 = 1;
const BORDER_COLOR: i32 = YELLOW;

// Location and color of SCORE information
const SCORE_X:      i32 = 1;
const SCORE_Y:      i32 = 2;
const SCORE_COLOR:  i32 = GREEN;

// Location and color of help information
const HELP_X:       i32 = 58;
const HELP_Y:       i32 = 1;
const HELP_COLOR:   i32 = CYAN;

// Next piece location
const NEXT_X:       i32 = 14;
const NEXT_Y:       i32 = 11;

// Location of "game over" in the end of the game
const GAMEOVER_X:   i32 = 1;
const GAMEOVER_Y:   i32 = PLAYFIELD_H + 3;

// Intervals after which game level (and game speed) is increased 
const LEVEL_UP:     i32 = 20;

const colors: [i32; 7] = [RED, GREEN, YELLOW, BLUE, FUCHSIA, CYAN, WHITE];

const empty_cell:   &str = " .";    // how we draw empty cell
const filled_cell:  &str = "[]";    // how we draw filled cell

proc_var!(use_color, bool, true);   // true if we use color, false if not
proc_var!(score, i32, 0);           // score variable initialization
proc_var!(level, i32, 1);           // level variable initialization
proc_var!(lines_completed, i32, 0); // completed lines counter initialization
// screen_buffer is variable, that accumulates all screen changes
// this variable is printed in controller once per game cycle
proc_var!(screen_buffer, String, "".to_string());

fn puts(changes: &str) {
    proc_var_set!(screen_buffer, |s| s.push_str(changes));
}

fn flush_screen() {
    eprint!("{}", proc_var_get!(screen_buffer));
    proc_var_set!(screen_buffer, |s| s.clear());
}

const ESC: char = '\x1b'; // escape key, '\033' in bash or c

// move cursor to (x,y) and print string
// (1,1) is upper left corner of the screen
fn xyprint(x: i32, y: i32, s: &str) {
    puts(&format!("{}[{};{}H{}", ESC, y, x, s));
}

fn show_cursor() {
    eprint!("{}[?25h", ESC);
}

fn hide_cursor() {
    eprint!("{}[?25l", ESC);
}

// foreground color
fn set_fg(color: i32) {
    if proc_var_get!(use_color) {
        puts(&format!("{}[3{}m", ESC, color));
    }
}

// background color
fn set_bg(color: i32) {
    if proc_var_get!(use_color) {
        puts(&format!("{}[4{}m", ESC, color));
    }
}

fn reset_colors() {
    puts(&format!("{}[0m", ESC));
}

fn set_bold() {
    puts(&format!("{}[1m", ESC));
}

// playfield is an array, each row is represented by integer
// each cell occupies 3 bits (empty if 0, other values encode color)
// playfield is initialized with 0s (empty cells)
proc_var!(playfield, [i32; PLAYFIELD_H as usize], [0; PLAYFIELD_H as usize]);

fn redraw_playfield() {
    for y in 0..PLAYFIELD_H {
        xyprint(PLAYFIELD_X, PLAYFIELD_Y + y, "");
        for x in 0..PLAYFIELD_W {
            let color = (proc_var_get!(playfield)[y as usize] >> (x * 3)) & 7;
            if color == 0 {
                puts(empty_cell);
            } else {
                set_fg(color);
                set_bg(color);
                puts(filled_cell);
                reset_colors();
            }
        }
    }
}

// Arguments: lines - number of completed lines
fn update_score(lines: i32) {
    proc_var_set!(lines_completed, |l| *l += lines);
    // Unfortunately I don't know scoring algorithm of original tetris
    // Here score is incremented with squared number of lines completed
    // this seems reasonable since it takes more efforts to complete several lines at once
    proc_var_set!(score, |s| *s += lines * lines);
    if proc_var_get!(score) > LEVEL_UP * proc_var_get!(level) { // if level should be increased
        proc_var_set!(level, |l| *l += 1);              // increment level
        proc_var_set!(DELAY, |d| *d *= DELAY_FACTOR);   // delay decreased
    }
    set_bold();
    set_fg(SCORE_COLOR);
    xyprint(SCORE_X, SCORE_Y, &format!("Lines completed: {}", proc_var_get!(lines_completed)));
    xyprint(SCORE_X, SCORE_Y + 1, &format!("Level:           {}", proc_var_get!(level)));
    xyprint(SCORE_X, SCORE_Y + 2, &format!("Score:           {}", proc_var_get!(score)));
    reset_colors();
}

const help: [&str; 9] = [
"  Use cursor keys",
"       or",
"      k: rotate",
"h: left,  l: right",
"      j: drop",
"      q: quit",
"  c: toggle color",
"n: toggle show next",
"H: toggle this help",
];

proc_var!(help_on, i32, 1); // if this flag is 1 help is shown

fn draw_help() {
    set_bold();
    set_fg(HELP_COLOR);
    for (i, &h) in help.iter().enumerate() {
        // ternary assignment: if help_on is 1 use string as is,
        // otherwise substitute all characters with spaces
        let s = if proc_var_get!(help_on) == 1 {
            h.to_owned()
        } else {
            " ".repeat(h.len())
        };
        xyprint(HELP_X, HELP_Y + i as i32, &s);
    }
    reset_colors();
}

fn toggle_help() {
    proc_var_set!(help_on, |h| *h ^= 1);
    draw_help();
}

// this array holds all possible pieces that can be used in the game
// each piece consists of 4 cells numbered from 0x0 to 0xf:
// 0123
// 4567
// 89ab
// cdef
// each string is sequence of cells for different orientations
// depending on piece symmetry there can be 1, 2 or 4 orientations
// relative coordinates are calculated as follows:
// x=((cell & 3)); y=((cell >> 2))
const piece_data: [&str; 7] = [
"1256",            // square
"159d4567",        // line
"45120459",        // s
"01561548",        // z
"159a845601592654",// l
"159804562159a654",// inverted l
"1456159645694159",// t
];

fn draw_piece(x: i32, y: i32, ctype: i32, rotation: i32, cell: &str) {
    // loop through piece cells: 4 cells, each has 2 coordinates
    for i in 0..4 {
        let c = piece_data[ctype as usize]
            .chars()
            .nth((i + rotation * 4) as usize)
            .unwrap()
            .to_digit(16)
            .unwrap() as i32;
        // relative coordinates are retrieved based on orientation and added to absolute coordinates
        let nx = x + (c & 3) * 2;
        let ny = y + (c >> 2);
        xyprint(nx, ny, cell);
    }
}

proc_var!(next_piece, i32, 0);
proc_var!(next_piece_rotation, i32, 0);
proc_var!(next_piece_color, i32, 0);

proc_var!(next_on, i32, 1); // if this flag is 1 next piece is shown

// Argument: visible - visibility (0 - no, 1 - yes),
// if this argument is skipped $next_on is used
fn draw_next(visible: i32) {
    let mut s = filled_cell.to_string();
    if visible == 1 {
        set_fg(proc_var_get!(next_piece_color));
        set_bg(proc_var_get!(next_piece_color));
    } else {
        s = " ".repeat(s.len());
    }
    draw_piece(NEXT_X, NEXT_Y, proc_var_get!(next_piece), proc_var_get!(next_piece_rotation), &s);
    reset_colors();
}

fn toggle_next() {
    proc_var_set!(next_on, |x| *x ^= 1);
    draw_next(proc_var_get!(next_on));
}

proc_var!(current_piece, i32, 0);
proc_var!(current_piece_x, i32, 0);
proc_var!(current_piece_y, i32, 0);
proc_var!(current_piece_color, i32, 0);
proc_var!(current_piece_rotation, i32, 0);

// Arguments: cell - string to draw single cell
fn draw_current(cell: &str) {
    // factor 2 for x because each cell is 2 characters wide
    draw_piece(proc_var_get!(current_piece_x) * 2 + PLAYFIELD_X,
               proc_var_get!(current_piece_y) + PLAYFIELD_Y,
               proc_var_get!(current_piece),
               proc_var_get!(current_piece_rotation),
               cell);
}

fn show_current() {
    set_fg(proc_var_get!(current_piece_color));
    set_bg(proc_var_get!(current_piece_color));
    draw_current(filled_cell);
    reset_colors();
}

fn clear_current() {
    draw_current(empty_cell);
}

// Arguments: x_test - new x coordinate of the piece, y_test - new y coordinate of the piece
// test if piece can be moved to new location
fn new_piece_location_ok(x_test: i32, y_test: i32) -> bool {
    for i in 0..4 {
        let c = piece_data[proc_var_get!(current_piece) as usize]
            .chars()
            .nth((i + proc_var_get!(current_piece_rotation) * 4) as usize)
            .unwrap()
            .to_digit(16)
            .unwrap() as i32;
        // new x and y coordinates of piece cell
        let y = (c >> 2) + y_test;
        let x = (c & 3) + x_test;
        // check if we are out of the play field
        if y < 0 || y >= PLAYFIELD_H || x < 0 || x >= PLAYFIELD_W {
            return false;
        }
        // check if location is already ocupied
        if ((proc_var_get!(playfield)[y as usize] >> (x * 3)) & 7) != 0 {
            return false;
        }
    }
    true
}

proc_var!(rands, VecDeque<u8>, VecDeque::new());
fn init_rands() {
    use std::iter::FromIterator;
    let mut f = std::fs::File::open("/dev/urandom").unwrap();
    let mut buffer = [0; 1024];
    f.read(&mut buffer).unwrap();
    proc_var_set!(rands, |r| *r = VecDeque::from_iter(buffer.iter().map(|c| c.to_owned())));
}

fn get_rand() -> u8 {
    if proc_var_get!(rands).is_empty() {
        init_rands();
    }
    let mut ret: u8 = 0;
    proc_var_set!(rands, |r| ret = r.pop_front().unwrap());
    ret
}

fn get_random_next() {
    // next piece becomes current
    proc_var_set!(current_piece, |cur| *cur = proc_var_get!(next_piece));
    proc_var_set!(current_piece_rotation, |cur| *cur = proc_var_get!(next_piece_rotation));
    proc_var_set!(current_piece_color, |cur| *cur = proc_var_get!(next_piece_color));
    // place current at the top of play field, approximately at the center
    proc_var_set!(current_piece_x, |cur| *cur = (PLAYFIELD_W - 4) / 2);
    proc_var_set!(current_piece_y, |cur| *cur = 0);
    // check if piece can be placed at this location, if not - game over
    if !new_piece_location_ok(
        proc_var_get!(current_piece_x),
        proc_var_get!(current_piece_y)) {
        cmd_exit();
    }
    show_current();

    draw_next(0);
    // now let's get next piece
    proc_var_set!(next_piece, |nxt| *nxt = (get_rand() % piece_data.len() as u8) as i32);
    let rotations = piece_data[proc_var_get!(next_piece) as usize].len() / 4;
    proc_var_set!(next_piece_rotation, |nxt| *nxt = (get_rand() % rotations as u8) as i32);
    proc_var_set!(next_piece_color, |nxt| *nxt = colors[(get_rand() as usize) % colors.len()]);
    draw_next(proc_var_get!(next_on));
}

fn draw_border() {
    set_bold();
    set_fg(BORDER_COLOR);
    let x1 = PLAYFIELD_X - 2;               // 2 here is because border is 2 characters thick
    let x2 = PLAYFIELD_X + PLAYFIELD_W * 2; // 2 here is because each cell on play field is 2 characters wide
    for i in 0..=PLAYFIELD_H {
        let y = i + PLAYFIELD_Y;
        xyprint(x1, y, "<|");
        xyprint(x2, y, "|>");
    }

    let y = PLAYFIELD_Y + PLAYFIELD_H;
    for i in 0..PLAYFIELD_W {
        let x1 = i * 2 + PLAYFIELD_X; // 2 here is because each cell on play field is 2 characters wide
        xyprint(x1, y, "==");
        xyprint(x1, y + 1, "\\/");
    }
    reset_colors();
}

fn redraw_screen() {
    draw_next(1);
    update_score(0);
    draw_help();
    draw_border();
    redraw_playfield();
    show_current();
}

fn toggle_color() {
    proc_var_set!(use_color, |x| *x = !*x);
    redraw_screen();
}

fn init() {
    run_cmd("clear").unwrap();
    init_rands();
    hide_cursor();
    get_random_next();
    get_random_next();
    redraw_screen();
    flush_screen();
}

// this function updates occupied cells in playfield array after piece is dropped
fn flatten_playfield() {
    for i in 0..4 {
        let c: i32 = piece_data[proc_var_get!(current_piece) as usize]
            .chars()
            .nth((i + proc_var_get!(current_piece_rotation) * 4) as usize)
            .unwrap()
            .to_digit(16)
            .unwrap() as i32;
        let y = (c >> 2) + proc_var_get!(current_piece_y);
        let x = (c & 3) + proc_var_get!(current_piece_x);
        proc_var_set!(playfield, |f| f[y as usize] |=
                      proc_var_get!(current_piece_color) << (x * 3));
    }
}

// this function takes row number as argument and checks if has empty cells
fn line_full(y: i32) -> bool {
    let row = proc_var_get!(playfield)[y as usize];
    for x in 0..PLAYFIELD_W {
        if ((row >> (x * 3)) & 7) == 0 {
            return false;
        }
    }
    true
}

// this function goes through playfield array and eliminates lines without empty cells
fn process_complete_lines() -> i32 {
    let mut complete_lines = 0;
    let mut last_idx = PLAYFIELD_H - 1;
    for y in (0..PLAYFIELD_H).rev() {
        if !line_full(y) {
            if last_idx != y {
                proc_var_set!(playfield, |f| f[last_idx as usize] = f[y as usize]);
            }
            last_idx -= 1;
        } else {
            complete_lines += 1;
        }
    }
    for y in 0..complete_lines {
        proc_var_set!(playfield, |f| f[y as usize] = 0);
    }
    complete_lines
}

fn process_fallen_piece() {
    flatten_playfield();
    let lines = process_complete_lines();
    if lines == 0 {
        return;
    } else {
        update_score(lines);
    }
    redraw_playfield();
}

// arguments: nx - new x coordinate, ny - new y coordinate
fn move_piece(nx: i32, ny: i32) -> bool {
    // moves the piece to the new location if possible
    if new_piece_location_ok(nx, ny) {      // if new location is ok
        clear_current();                    // let's wipe out piece current location
        proc_var_set!(current_piece_x,      // update x ...
                      |x| *x = nx);
        proc_var_set!(current_piece_y,      // ... and y of new location
                      |y| *y = ny);
        show_current();                     // and draw piece in new location
        return true;                        // nothing more to do here
    }                                       // if we could not move piece to new location
    if ny == proc_var_get!(current_piece_y) {
        return true;                        // and this was not horizontal move
    }
    process_fallen_piece();                 // let's finalize this piece
    get_random_next();                      // and start the new one
    false
}

fn cmd_right() {
    move_piece(proc_var_get!(current_piece_x) + 1,
               proc_var_get!(current_piece_y));
}

fn cmd_left() {
    move_piece(proc_var_get!(current_piece_x) - 1,
               proc_var_get!(current_piece_y));
}

fn cmd_rotate() {
    // local available_rotations old_rotation new_rotation
    // number of orientations for this piece
    let available_rotations = piece_data[proc_var_get!(current_piece) as usize].len() as i32 / 4;
    let old_rotation = proc_var_get!(current_piece_rotation);           // preserve current orientation
    let new_rotation = (old_rotation + 1) % available_rotations;        // calculate new orientation
    proc_var_set!(current_piece_rotation, |r| *r = new_rotation);       // set orientation to new
    if new_piece_location_ok(proc_var_get!(current_piece_x),            // check if new orientation is ok
                             proc_var_get!(current_piece_y)) {
        proc_var_set!(current_piece_rotation, |r| *r = old_rotation);   // if yes - restore old orientation
        clear_current();                                                // clear piece image
        proc_var_set!(current_piece_rotation, |r| *r = new_rotation);   // set new orientation
        show_current();                                                 // draw piece with new orientation
    } else {                                                            // if new orientation is not ok
        proc_var_set!(current_piece_rotation, |r| *r = old_rotation);   // restore old orientation
    }
}

fn cmd_down() {
    move_piece(proc_var_get!(current_piece_x),
               proc_var_get!(current_piece_y) + 1);
}

fn cmd_drop() {
    // move piece all way down
    // loop body is empty
    // loop condition is done at least once
    // loop runs until loop condition would return non zero exit code
    loop {
        if !move_piece(proc_var_get!(current_piece_x),
                       proc_var_get!(current_piece_y) + 1) {
            break;
        }
    }
}

proc_var!(old_stty_cfg, String, String::new());

fn cmd_exit() {
    xyprint(GAMEOVER_X, GAMEOVER_Y, "Game over!");
    xyprint(GAMEOVER_X, GAMEOVER_Y + 1, "");// reset cursor position
    flush_screen();                         // ... print final message ...
    show_cursor();
    let stty_g = proc_var_get!(old_stty_cfg);
    run_cmd(format!("stty {}", stty_g)).unwrap();   // ... and restore terminal state
    std::process::exit(0);
}

fn main() -> CmdResult {
    let old_cfg = run_fun("stty -g")?;  // let's save terminal state ...
    proc_var_set!(old_stty_cfg, |cfg| *cfg = old_cfg);
    run_cmd("stty raw -echo -isig -icanon min 0 time 0")?;

    init();
    let mut tick = 0;
    loop {
        let mut buffer = String::new();
        if std::io::stdin().read_to_string(&mut buffer).is_ok() {
            match buffer.as_str() {
                "q" | "\u{1b}" | "\u{3}" => cmd_exit(), // q, ESC or Ctrl-C to exit
                "h" | "\u{1b}[D"  => cmd_left(),
                "l" | "\u{1b}[C"  => cmd_right(),
                "j" | "\u{1b}[B"  => cmd_drop(),
                "k" | "\u{1b}[A"  => cmd_rotate(),
                "H" => toggle_help(),
                "n" => toggle_next(),
                "c" => toggle_color(),
                _ => (),
            }
        }
        tick += 1;
        if tick >= (600.0 * proc_var_get!(DELAY)) as i32 {
            tick = 0;
            cmd_down();
        }
        flush_screen();
        thread::sleep(time::Duration::from_millis(1));
    }
}
