# Sonar
## Todo Feat
- Git integration? Maybe rely on git and ignore if not
- Select alt, like right click for context (delete, multi, etc)?
- Better input handling (especially for commands)
- Scroll/progress bars
- Shortcuts / bookmarks
- File size
- Line count
- Modified
- Lazy loading - delay before loading file content and metadata
- Search delay - delay before filtering
- Hide preview window
- If preview window hidden dont generate preview or meta
- Toggle hidden files
- Mouse support?
- Seer?

## Todo Cmd
- clipboard-file-contents
- clipboard-file-path
- config-init
- config-clear

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
- when exploded there is really no need to check for mimetypes


## Notes


### Selection vs multi-selection
The difference between a selection (current list selection) and multi-select is confusing.

With tab bount to mul-sel it makes sense to not really use `select` for selections at all

But I need to change the verbage around that

Maybe mul-sel becomes checked or hilight or grab or context

I kind of like `ctx` for context but sel is clearer. 

Maybe `select` is what needs to change and `mul-sel` becomes `sel`

`select` could become enter or ok


## Features
- Fuzzy Find First
- Custom Keybinds
- Multi-select operations
- Responsive layout
- Optional bat integration
- Custom native shell commands
- Image preview
- Discoverable commands and features

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
