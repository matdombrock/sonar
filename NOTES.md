# Sonar
## Todo Feat
- Image previews
- Git integration? Maybe rely on git and ignore if not
- Select alt, like right click for context (delete, multi, etc)?
- Better input handling (especially for commands)
- Scroll/progress bars
- Config file
- Shortcuts / bookmarks
- Custom native commands
- File size
- Line count
- Modified
- Mouse support?
- Seer?

## Todo Cmd
- mul-move
- clipboard-file-contents
- clipboard-file-path
- keybind-init
- reload-dir

## Todo Bug / Incomplete
- Handle large dirs
- Fix file read not clearing (foot only)
- Use primary up/down for scrolling output
- Vis commands doesnt update selection at fist (same for cmd finder)
- Use users real shell instead of sh
- Shell commands dont operate in the correct directory
- Directories do not refresh after commands change them (mul-copy)
- Better keybind file handling
- Better keybind printing
- Load default keybinds from embedded text (parse)
- multi select edit should offer to do something

## Notes

### Custom commands
Custom commands can be loaded from a file. 

They are set up like normal commands but the cmd starts with `!`

These should pass in the selection list somehow. 

```
!ls $1
!ls $...
```

Neither of these are valid variables names. Which is good. 

### Finder as default
It would make sense for command finder to be the main way of launching commands
But then its not easily possible to run a command on a single selection
Maybe caching the selection while in command finder could work?
When a cmd is selected just send it to the cmd popup?

### Selection vs multi-selection
The difference between a selection (current list selection) and multi-select is confusing.

With tab bount to mul-sel it makes sense to not really use `select` for selections at all

But I need to change the verbage around that

Maybe mul-sel becomes checked or hilight or grab or context

I kind of like `ctx` for context but sel is clearer. 

Maybe `select` is what needs to change and `mul-sel` becomes `sel`

`select` could become enter or ok

### Command props
is_hidden
uses_single_sel
uses_multi_sel

### fn in cmd struct
Might be able to use functions in the cmd def type. Right now all cmd_* functions just take self. They might need to take some other params in the future but that could just be a space delimited string like a bash command. 

Command functions should never really need to return any values. They cant be piped or redirected really. If they have output they write it to "output".

One problem there though is that the commands operate on self (app) so if they move out of the app impl they will need to refer to an instance of app. 

### Config
- Set border colors
- Custom welcome message
- Sort - file first | dir first | alpha | chron | chron rev 


## Features
- Fuzzy Find First
- Custom Keybinds
- Multi-select operations
- Responsive layout
- Optional bat integration

## Goals

## VS FF
- Multi-select across dirs
- Floating/extra windows
- Resume location

## VS Yazi
- Search first
- Panes are confusing


## About
X is an ergonomic, friendly, "fuzzy first" file explorer for your terminal.
EFFFFE
