# <img src="res/kricon.svg" width="18"> kricon

kricon is a purpose-built icon editing utility.

## <img src="res/icons/ico_boring.svg" width="18"> Controls

| Key | Action |
| --- | --- |
| `G` | Translates the selection. |
| `R` | Rotates the selection. |
| `S` | Scales the selection. |
| `Shift` + `S` | Subdivides the edges between the selected vertices. |
| `Ctrl` + `S` | Subdivides the edges between the selected vertices along a curve fitted to the neighbouring vertices. |
| `Alt` + `S` | Decimates the selected vertices. Removes every second vertex along each chain of selected vertices, the reverse of one subdivision. |
| `H` | Spaces the selected vertices evenly along each chain of selected vertices. The ends of each chain stay in place. |
| `X` / `Y` | Locks the current transform to the X or Y axis. |
| `Alt` | Snaps the translated selection to the grid while held. |
| `Shift` | Snaps the translated vertices to the X or Y of a nearby vertex while held. A guide line shows each snap. |
| `0`-`9` / `-` | Rotates by the typed angle in degrees. A negative angle rotates clockwise. |
| `Enter` | Confirms the current transform. |
| `Escape` | Cancels the current transform or closes the open menu. |
| `E` | Extrudes the selected vertices. |
| `Shift` + `D` | Duplicates the selected vertices. |
| `F` | Connects the two selected vertices with an edge. |
| `V` | Adds a vertex at the cursor. |
| `Right Click` | Selects the vertex, face or image at the cursor. |
| `C` | Toggles brush selection. `Scroll` sets the size, `Left Click` selects and `Middle Click` deselects. |
| `B` | Starts box selection. Drag with `Left Click` to select the vertices in the box. |
| `L` | Selects all linked vertices to the currently selected. |
| `W` | Opens the create menu. |
| `Shift` + `Scroll` | Scales the new primitive before placement. `Scroll` sets the vertex count of a new circle or curve. |
| `M` | Opens the merge menu. |
| `X` | Dissolves the selected vertices and keeps the line between their neighbours. |
| `P` | Toggles a hole in the faces enclosed by the selected vertices. |
| `Y` | Opens a colour palette at the cursor to set the colour of the selected faces. `Enter` or `Left Click` outside confirms, `Escape` cancels. |
| `Ctrl` + `Y` | Copies the colour of the selected face to the clipboard as a hex code. |
| `Shift` + `Y` | Sets the colour of the selected faces from a `#XXXXXX` or `#XXX` hex code in the clipboard. |
| `Delete` | Deletes the selection. |

## <img src="res/icons/ico_boring.svg" width="18"> Legal

kricon is licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE) for the full text.
