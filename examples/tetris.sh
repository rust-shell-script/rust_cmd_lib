#!/bin/bash

# Tetris game written in pure bash
#
# I tried to mimic as close as possible original tetris game
# which was implemented on old soviet DVK computers (PDP-11 clones)
#
# Videos of this tetris can be found here:
#
# http://www.youtube.com/watch?v=O0gAgQQHFcQ
# http://www.youtube.com/watch?v=iIQc1F3UuV4
#
# This script was created on ubuntu 13.04 x64 and bash 4.2.45(1)-release.
# It was not tested on other unix like operating systems.
#
# Enjoy :-)!
#
# Author: Kirill Timofeev <kt97679@gmail.com>
#
# This program is free software. It comes without any warranty, to the extent
# permitted by applicable law. You can redistribute it and/or modify it under
# the terms of the Do What The Fuck You Want To Public License, Version 2, as
# published by Sam Hocevar. See http://www.wtfpl.net/ for more details.

set -u # non initialized variable is an error

# Those are commands sent to controller by key press processing code
# In controller they are used as index to retrieve actual functuon from array
QUIT=0
RIGHT=1
LEFT=2
ROTATE=3
DOWN=4
DROP=5
TOGGLE_HELP=6
TOGGLE_NEXT=7
TOGGLE_COLOR=8

DELAY=1000          # initial delay between piece movements (milliseconds)
DELAY_FACTOR="8/10" # this value controls delay decrease for each level up

# color codes
RED=1
GREEN=2
YELLOW=3
BLUE=4
FUCHSIA=5
CYAN=6
WHITE=7

# Location and size of playfield, color of border
PLAYFIELD_W=10
PLAYFIELD_H=20
PLAYFIELD_X=30
PLAYFIELD_Y=1
BORDER_COLOR=$YELLOW

# Location and color of score information
SCORE_X=1
SCORE_Y=2
SCORE_COLOR=$GREEN

# Location and color of help information
HELP_X=58
HELP_Y=1
HELP_COLOR=$CYAN

# Next piece location
NEXT_X=14
NEXT_Y=11

# Location of "game over" in the end of the game
GAMEOVER_X=1
GAMEOVER_Y=$((PLAYFIELD_H + 3))

# Intervals after which game level (and game speed) is increased 
LEVEL_UP=20

colors=($RED $GREEN $YELLOW $BLUE $FUCHSIA $CYAN $WHITE)

use_color=1      # 1 if we use color, 0 if not
empty_cell=" ."  # how we draw empty cell
filled_cell="[]" # how we draw filled cell

score=0           # score variable initialization
level=1           # level variable initialization
lines_completed=0 # completed lines counter initialization

# screen_buffer is variable, that accumulates all screen changes
# this variable is printed in controller once per game cycle
screen_buffer=""
puts() {
    screen_buffer+=${1}
}

flush_screen() {
    echo -ne "$screen_buffer"
    screen_buffer=""
}

# move cursor to (x,y) and print string
# (1,1) is upper left corner of the screen
xyprint() {
    puts "\e[${2};${1}H${3}"
}

show_cursor() {
    echo -ne "\e[?25h"
}

hide_cursor() {
    echo -ne "\e[?25l"
}

# foreground color
set_fg() {
    ((use_color)) && puts "\e[3${1}m"
}

# background color
set_bg() {
    ((use_color)) && puts "\e[4${1}m"
}

reset_colors() {
    puts "\e[0m"
}

set_bold() {
    puts "\e[1m"
}

# playfield is an array, each row is represented by integer
# each cell occupies 3 bits (empty if 0, other values encode color)
redraw_playfield() {
    local x y color

    for ((y = 0; y < PLAYFIELD_H; y++)) {
        xyprint $PLAYFIELD_X $((PLAYFIELD_Y + y)) ""
        for ((x = 0; x < PLAYFIELD_W; x++)) {
            ((color = ((playfield[y] >> (x * 3)) & 7)))
            if ((color == 0)) ; then
                puts "$empty_cell"
            else
                set_fg $color
                set_bg $color
                puts "$filled_cell"
                reset_colors
            fi
        }
    }
}

update_score() {
    # Arguments: 1 - number of completed lines
    ((lines_completed += $1))
    # Unfortunately I don't know scoring algorithm of original tetris
    # Here score is incremented with squared number of lines completed
    # this seems reasonable since it takes more efforts to complete several lines at once
    ((score += ($1 * $1)))
    if (( score > LEVEL_UP * level)) ; then          # if level should be increased
        ((level++))                                  # increment level
        kill -SIGUSR1 $ticker_pid # and send SIGUSR1 signal to ticker process (please see ticker() function for more details)
    fi
    set_bold
    set_fg $SCORE_COLOR
    xyprint $SCORE_X $SCORE_Y         "Lines completed: $lines_completed"
    xyprint $SCORE_X $((SCORE_Y + 1)) "Level:           $level"
    xyprint $SCORE_X $((SCORE_Y + 2)) "Score:           $score"
    reset_colors
}

help=(
"  Use cursor keys"
"       or"
"    s: rotate"
"a: left,  d: right"
"    space: drop"
"      q: quit"
"  c: toggle color"
"n: toggle show next"
"h: toggle this help"
)

help_on=1 # if this flag is 1 help is shown

draw_help() {
    local i s

    set_bold
    set_fg $HELP_COLOR
    for ((i = 0; i < ${#help[@]}; i++ )) {
        # ternary assignment: if help_on is 1 use string as is, otherwise substitute all characters with spaces
        ((help_on)) && s="${help[i]}" || s="${help[i]//?/ }"
        xyprint $HELP_X $((HELP_Y + i)) "$s"
    }
    reset_colors
}

toggle_help() {
    ((help_on ^= 1))
    draw_help
}

# this array holds all possible pieces that can be used in the game
# each piece consists of 4 cells numbered from 0x0 to 0xf:
# 0123
# 4567
# 89ab
# cdef
# each string is sequence of cells for different orientations
# depending on piece symmetry there can be 1, 2 or 4 orientations
# relative coordinates are calculated as follows:
# x=((cell & 3)); y=((cell >> 2))
piece_data=(
"1256"             # square
"159d4567"         # line
"45120459"         # s
"01561548"         # z
"159a845601592654" # l
"159804562159a654" # inverted l
"1456159645694159" # t
)

draw_piece() {
    # Arguments:
    # 1 - x, 2 - y, 3 - type, 4 - rotation, 5 - cell content
    local i x y c

    # loop through piece cells: 4 cells, each has 2 coordinates
    for ((i = 0; i < 4; i++)) {
        c=0x${piece_data[$3]:$((i + $4 * 4)):1}
        # relative coordinates are retrieved based on orientation and added to absolute coordinates
        ((x = $1 + (c & 3) * 2))
        ((y = $2 + (c >> 2)))
        xyprint $x $y "$5"
    }
}

next_piece=0
next_piece_rotation=0
next_piece_color=0

next_on=1 # if this flag is 1 next piece is shown

draw_next() {
    # Argument: 1 - visibility (0 - no, 1 - yes), if this argument is skipped $next_on is used
    local s="$filled_cell" visible=${1:-$next_on}
    ((visible)) && {
        set_fg $next_piece_color
        set_bg $next_piece_color
    } || {
        s="${s//?/ }"
    }
    draw_piece $NEXT_X $NEXT_Y $next_piece $next_piece_rotation "$s"
    reset_colors
}

toggle_next() {
    draw_next $((next_on ^= 1))
}

draw_current() {
    # Arguments: 1 - string to draw single cell
    # factor 2 for x because each cell is 2 characters wide
    draw_piece $((current_piece_x * 2 + PLAYFIELD_X)) $((current_piece_y + PLAYFIELD_Y)) $current_piece $current_piece_rotation "$1"
}

show_current() {
    set_fg $current_piece_color
    set_bg $current_piece_color
    draw_current "${filled_cell}"
    reset_colors
}

clear_current() {
    draw_current "${empty_cell}"
}

new_piece_location_ok() {
    # Arguments: 1 - new x coordinate of the piece, 2 - new y coordinate of the piece
    # test if piece can be moved to new location
    local i c x y x_test=$1 y_test=$2

    for ((i = 0; i < 4; i++)) {
        c=0x${piece_data[$current_piece]:$((i + current_piece_rotation * 4)):1}
        # new x and y coordinates of piece cell
        ((y = (c >> 2) + y_test))
        ((x = (c & 3) + x_test))
        ((y < 0 || y >= PLAYFIELD_H || x < 0 || x >= PLAYFIELD_W )) && return 1 # check if we are out of the play field
        ((((playfield[y] >> (x * 3)) & 7) != 0 )) && return 1                  # check if location is already ocupied
    }
    return 0
}

get_random_next() {
    # next piece becomes current
    current_piece=$next_piece
    current_piece_rotation=$next_piece_rotation
    current_piece_color=$next_piece_color
    # place current at the top of play field, approximately at the center
    ((current_piece_x = (PLAYFIELD_W - 4) / 2))
    ((current_piece_y = 0))
    # check if piece can be placed at this location, if not - game over
    new_piece_location_ok $current_piece_x $current_piece_y || exit
    show_current

    draw_next 0
    # now let's get next piece
    ((next_piece = RANDOM % ${#piece_data[@]}))
    ((next_piece_rotation = RANDOM % (${#piece_data[$next_piece]} / 4)))
    ((next_piece_color = colors[RANDOM % ${#colors[@]}]))
    draw_next
}

draw_border() {
    local i x1 x2 y

    set_bold
    set_fg $BORDER_COLOR
    ((x1 = PLAYFIELD_X - 2))               # 2 here is because border is 2 characters thick
    ((x2 = PLAYFIELD_X + PLAYFIELD_W * 2)) # 2 here is because each cell on play field is 2 characters wide
    for ((i = 0; i < PLAYFIELD_H + 1; i++)) {
        ((y = i + PLAYFIELD_Y))
        xyprint $x1 $y "<|"
        xyprint $x2 $y "|>"
    }

    ((y = PLAYFIELD_Y + PLAYFIELD_H))
    for ((i = 0; i < PLAYFIELD_W; i++)) {
        ((x1 = i * 2 + PLAYFIELD_X)) # 2 here is because each cell on play field is 2 characters wide
        xyprint $x1 $y '=='
        xyprint $x1 $((y + 1)) "\/"
    }
    reset_colors
}

redraw_screen() {
    draw_next
    update_score 0
    draw_help
    draw_border
    redraw_playfield
    show_current
}

toggle_color() {
    ((use_color ^= 1))
    redraw_screen
}

init() {
    local i

    # playfield is initialized with -1s (empty cells)
    for ((i = 0; i < PLAYFIELD_H; i++)) {
        playfield[$i]=0
    }

    clear
    hide_cursor
    get_random_next
    get_random_next
    redraw_screen
    flush_screen
}

# this function updates occupied cells in playfield array after piece is dropped
flatten_playfield() {
    local i c x y
    for ((i = 0; i < 4; i++)) {
        c=0x${piece_data[$current_piece]:$((i + current_piece_rotation * 4)):1}
        ((y = (c >> 2) + current_piece_y))
        ((x = (c & 3) + current_piece_x))
        ((playfield[y] |= (current_piece_color << (x * 3))))
    }
}

# this function takes row number as argument and checks if has empty cells
line_full() {
    local row=${playfield[$1]} x
    for ((x = 0; x < PLAYFIELD_W; x++)) {
        ((((row >> (x * 3)) & 7) == 0)) && return 1
    }
    return 0
}

# this function goes through playfield array and eliminates lines without empty cells
process_complete_lines() {
    local y complete_lines=0
    for ((y = PLAYFIELD_H - 1; y > -1; y--)) {
        line_full $y && {
            unset playfield[$y]
            ((complete_lines++))
        }
    }
    for ((y = 0; y < complete_lines; y++)) {
        playfield=(0 ${playfield[@]})
    }
    return $complete_lines
}

process_fallen_piece() {
    flatten_playfield
    process_complete_lines && return
    update_score $?
    redraw_playfield
}

move_piece() {
# arguments: 1 - new x coordinate, 2 - new y coordinate
# moves the piece to the new location if possible
    if new_piece_location_ok $1 $2 ; then # if new location is ok
        clear_current                     # let's wipe out piece current location
        current_piece_x=$1                # update x ...
        current_piece_y=$2                # ... and y of new location
        show_current                      # and draw piece in new location
        return 0                          # nothing more to do here
    fi                                    # if we could not move piece to new location
    (($2 == current_piece_y)) && return 0 # and this was not horizontal move
    process_fallen_piece                  # let's finalize this piece
    get_random_next                       # and start the new one
    return 1
}

cmd_right() {
    move_piece $((current_piece_x + 1)) $current_piece_y
}

cmd_left() {
    move_piece $((current_piece_x - 1)) $current_piece_y
}

cmd_rotate() {
    local available_rotations old_rotation new_rotation

    available_rotations=$((${#piece_data[$current_piece]} / 4))       # number of orientations for this piece
    old_rotation=$current_piece_rotation                              # preserve current orientation
    new_rotation=$(((old_rotation + 1) % available_rotations))        # calculate new orientation
    current_piece_rotation=$new_rotation                              # set orientation to new
    if new_piece_location_ok $current_piece_x $current_piece_y ; then # check if new orientation is ok
        current_piece_rotation=$old_rotation                          # if yes - restore old orientation
        clear_current                                                 # clear piece image
        current_piece_rotation=$new_rotation                          # set new orientation
        show_current                                                  # draw piece with new orientation
    else                                                              # if new orientation is not ok
        current_piece_rotation=$old_rotation                          # restore old orientation
    fi
}

cmd_down() {
    move_piece $current_piece_x $((current_piece_y + 1))
}

cmd_drop() {
    # move piece all way down
    # this is example of do..while loop in bash
    # loop body is empty
    # loop condition is done at least once
    # loop runs until loop condition would return non zero exit code
    while move_piece $current_piece_x $((current_piece_y + 1)) ; do : ; done
}

stty_g=$(stty -g)              # let's save terminal state ...

at_exit() {
    kill $ticker_pid                             # let's kill ticker process ...
    xyprint $GAMEOVER_X $GAMEOVER_Y "Game over!"
    echo -e "$screen_buffer"                     # ... print final message ...
    show_cursor
    stty $stty_g                                 # ... and restore terminal state
}

# this function runs in separate process
# it sends SIGUSR1 signals to the main process with appropriate delay
ticker() {
    # on SIGUSR1 delay should be decreased, this happens during level ups
    trap 'DELAY=$(($DELAY * $DELAY_FACTOR))' SIGUSR1
    trap exit TERM

    while sleep $((DELAY / 1000)).$(printf "%03d" $((DELAY % 1000))); do kill -SIGUSR1 $1 || exit; done 2>/dev/null
}

do_tick() {
    $tick_blocked && tick_scheduled=true && return
    cmd_down
    flush_screen
}

main() {
    local -u key a='' b='' esc_ch=$'\x1b'
    local cmd
    # commands is associative array, which maps pressed keys to commands, sent to controller
    local -A commands=([A]=cmd_rotate [C]=cmd_right [D]=cmd_left
        [_S]=cmd_rotate [_A]=cmd_left [_D]=cmd_right
        [_]=cmd_drop [_Q]=exit [_H]=toggle_help [_N]=toggle_next [_C]=toggle_color)

    trap at_exit EXIT
    trap do_tick SIGUSR1
    init
    ticker $$ &
    ticker_pid=$!
    tick_blocked=false
    tick_scheduled=false

    while read -s -n 1 key ; do
        case "$a$b$key" in
            "${esc_ch}["[ACD]) cmd=${commands[$key]} ;; # cursor key
            *${esc_ch}${esc_ch}) cmd=exit ;;            # exit on 2 escapes
            *) cmd=${commands[_$key]:-} ;;              # regular key. If space was pressed $key is empty
        esac
        a=$b   # preserve previous keys
        b=$key
        [ -n "$cmd" ] && {
            tick_blocked=true
            $cmd
            tick_blocked=false
            $tick_scheduled && tick_scheduled=false && do_tick
            flush_screen
        }
    done
}

main
