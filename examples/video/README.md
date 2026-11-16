# Anikit Video examples

| File | What |
|---|---|
| `hello.json` | The smallest useful spec: `anikit-video render hello.json -o hello.mp4` |
| `brand.json` | A brand kit (colours, fonts, voice) |
| `launch/params.json` | Parameters for the `launch` template (Anikit Video's own launch film) |
| `build.mjs` | The TypeScript/JS builder (`anikit/video`): a hand-built teaser plus the launch template in 16:9, 9:16 and 1:1 |
| `build.py` | The same in Python (`anikit.video`) |
| `mcp.json` | MCP client configuration for `anikit-video mcp` |

```bash
cargo build --release -p anikit-video && export PATH=$PWD/target/release:$PATH
anikit-video templates expand launch examples/video/launch/params.json --brand examples/video/brand.json --format 9:16 -o /tmp/launch.json
anikit-video critique /tmp/launch.json
anikit-video sheet /tmp/launch.json -o /tmp/sheet.png && anikit-video render /tmp/launch.json -o launch.mp4
```

New project from a README: `anikit-video new launch --brand examples/video/brand.json --from README.md`. Then follow the generated `launch/NEXT.md`.
