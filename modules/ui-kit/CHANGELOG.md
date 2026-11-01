# Changelog — pwc.ui-kit

## 1.0.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 1.0.0

- First release. The menu framework (`Menu`, `View`, `Row`, `Framed`, `Cursor`, `drive`, `gather`),
  the default look (`DefaultTheme`, `present`, `draw_waiting`) and the text widgets (`TextInput`,
  `EditBuf`, `Ring`, `Completion`, `common_prefix`), moved out of the PWC core unchanged in
  behaviour. Screens draw into the core's `UiElement`s instead of the frame.
- A library: it registers nothing. Requires `pwc-api ^3.0`.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
