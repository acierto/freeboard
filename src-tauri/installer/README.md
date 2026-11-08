# Freeboard installer art

Put this folder in the repo as `src-tauri/installer/`.

| File | Used by | Size |
| --- | --- | --- |
| `dmg-background.png`, `dmg-background@2x.png` | macOS DMG window | 660 × 400 (and 1320 × 800) |
| `nsis-sidebar.bmp` | Windows installer Welcome and Finish pages | 164 × 314 |
| `nsis-header.bmp` | Windows installer page headers | 150 × 57 |
| `*.svg` | Sources, edit these and re-export | |
| `preview-dmg.png` | What the DMG window should look like | |

## macOS: one sharp background for normal and Retina screens

```sh
tiffutil -cathidpicheck dmg-background.png dmg-background@2x.png -out dmg-background.tiff
```

If the bundler rejects the TIFF, use `dmg-background.png` instead.

## tauri.conf.json

```json
{
  "bundle": {
    "macOS": {
      "dmg": {
        "background": "installer/dmg-background.tiff",
        "windowSize": { "width": 660, "height": 400 },
        "appPosition": { "x": 170, "y": 180 },
        "applicationFolderPosition": { "x": 490, "y": 180 }
      }
    },
    "windows": {
      "nsis": {
        "headerImage": "installer/nsis-header.bmp",
        "sidebarImage": "installer/nsis-sidebar.bmp",
        "installerIcon": "icons/icon.ico",
        "installMode": "currentUser",
        "languages": ["English", "Dutch", "German", "French", "Spanish"],
        "displayLanguageSelector": false
      }
    }
  }
}
```

Build the DMG on a Mac: icon positions are not applied when the DMG is made on CI (tauri-apps/tauri#1731).

## The license window

Remove the click-through license from the DMG. MIT does not require anyone to accept it, and that window is a fixed macOS dialog that can't be styled. Ship `LICENSE` inside the app and show it in About instead. If you keep it, unwrap the text so each paragraph is one line; the hard line breaks are what makes it look broken.
