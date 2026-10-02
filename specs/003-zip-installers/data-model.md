# Data Model: Linux Zip Distribution

No data. The artifact layout, from the spec (D-1):

```text
metagente-linux-amd64.zip
└── metagente/
    ├── README.md             small README written during assembly
    ├── metagente.toml        from the repository root
    ├── bin/
    │   └── metagente         target/release/metagente
    ├── docs/
    │   ├── README.md         the repository README.md
    │   ├── guide.md          docs/guide.md
    │   ├── syntax.md         docs/syntax.md
    │   └── tutorial.md       docs/tutorial.md
    └── samples/
        ├── clock.ag          examples/clock.ag
        ├── read_file.ag      examples/read_file.ag
        ├── weather.ag        examples/weather.ag
        ├── planner.ag        examples/planner.ag
        └── remote_weather.ag examples/remote_weather.ag
```

12 files, no folder entries. On Windows `bin/metagente.exe` replaces `bin/metagente`.

Version everywhere: v0.1.1 (README line, `Cargo.toml`, `CHANGELOG.md` heading, `--version` output, small README).
