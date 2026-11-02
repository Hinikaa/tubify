# rounded-polygon-qmljs

From https://github.com/end-4/rounded-polygon-qmljs at commit
e31ec4cb4ebf6a46b267f5c42eabf6874916fa16, the commit end-4/dots-hyprland
pins. Apache License 2.0 (LICENSE): a QML JavaScript port of Knugel's
TypeScript port of androidx's graphics-shapes, with Material 3's 35 shapes.

Only the library is kept (the examples and the Canvas-drawn ShapeCanvas are
left out). tools/shape_atlas reads material-shapes.js at build time to turn
every shape into a distance field for the app's MaterialShape shader.
