# Sonar
## Todo Feat
- Custom keybinds
- Image previews
- Tab to use primary up/down on preview?
- Git integration?
- Select alt, like right click for context (delete, multi, etc)?
- Better input handling (especially for commands)
- Seer?

## Todo Cmd
- multi-delete
- multi-move
- clipboard-file-contents
- clipboard-file-path

## Todo Bug / Incomplete
- Handle large dirs
- Fix file read not clearing
- Beter metadata handling
- Use primary up/down for scrolling output
- Vis commands doesnt update selection at fist (same for cmd finder)
- Use users real shell instead of sh
- Shell commands dont operate in the correct directory
- Directories do not refresh after commands change them

## Notes

### Finder as default
It would make sense for command finder to be the main way of launching commands
But then its not easily possible to run a command on a single selection
Maybe caching the selection while in command finder could work?
When a cmd is selected just send it to the cmd popup?

### Selection vs multi-selection
The difference between a selection (current list selection) and multi-select is confusing.

### Command props
is_hidden
uses_single_sel
uses_multi_sel

### fn in cmd struct
Might be able to use functions in the cmd def type. Right now all cmd_* functions just take self. They might need to take some other params in the future but that could just be a space delimited string like a bash command. 

Command functions should never really need to return any values. They cant be piped or redirected really. If they have output they write it to "output".

One problem there though is that the commands operate on self (app) so if they move out of the app impl they will need to refer to an instance of app. 


## Features
- Fuzzy Find First
- Custom Keybinds
- Multi-select
- Responsize design

## VS FF
- Multi-select across dirs
- Floating/extra windows
- Resume location
