# The keyboard

[Deutsch](keyboard.de.md)

AmberBeam is built to be used with the keyboard. Press **F1** in the program
to see the keys as they are set right now — that window reads the same table
the keys themselves come from, so it cannot be out of date.

## The Mac function key problem

On a Mac, F1 to F12 carry brightness and volume. Two separate things stand
between them and a program:

1. **A bare F5 is brightness.** `fn` + F5 reaches the program anyway, this
   minute, with nothing switched. One setting in System Settings ▸ Keyboard
   saves you holding `fn` — it is a convenience, not a requirement.
2. **F3, F4 and F11 belong to the system.** Mission Control, Spotlight and
   "show desktop" take them before any program sees them, whatever you hold.
   Each can be switched off on its own under Keyboard Shortcuts, or AmberBeam
   can simply use other keys for those three.

The dialog on the first start finds out which of these is in your way — by
asking you to press a key and seeing whether it arrives — and offers three
layouts.

## The three layouts

| | What it is | What it needs |
|---|---|---|
| **Windows style** | The function keys as they are on a PC | F3, F4 and F11 given up in System Settings |
| **Mixed** | Function keys where the system allows, ⌘ for the three it does not | Nothing |
| **Mac-friendly** | No function key at all | Nothing |

Every key in every layout can be changed: **Keys ▸ Change the keys**, click a
row, press the key you want. A key taken from another action says which. A
layout can be exported to a file and read back on another machine.

## What is never rebound

Moving about a list is not a matter of taste:

| | |
|---|---|
| ↑ ↓ | Move through the list |
| ↵ | Open a folder, enter a file |
| ⌫ | One folder up |
| Space | Select and move on |
| ⇞ ⇟ ⇱ ⇲ | Page, and jump to the ends |

These are what a list is. A layout that rebound them would be a layout nobody
could use.

## Why AmberBeam does not simply take the keys

It could. Catching keys below the operating system is possible — it needs the
accessibility permission, breaks with every macOS update, and makes a program
look like something that reads keystrokes. The price is higher than the gain.
