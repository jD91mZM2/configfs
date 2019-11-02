# configfs

Crazy project that uses FUSE to create virtual configs for all your
configs.

```sh
configfs test-mount/
ln -s my-config.toml test-mount/my-config.toml
ln -s my-config.json test-mount/my-config.json
ln -s my-config.yaml test-mount/my-config.yaml
```

This should have created a structure such as

```
test-mount
├── my-config.json/
│   ├── config.json
│   ├── config.toml
│   └── config.yml
├── my-config.toml/
│   ├── config.json
│   ├── config.toml
│   └── config.yml
└── my-config.yml/
    ├── config.json
    ├── config.toml
    └── config.yml

3 directories, 9 files
```

The `my-config.json` directory has 3 different variations of your
original configuration:

- One reformatted JSON config
- One TOML config
- One YAML config.

## Why this is a thing

Because it *can* be a thing, which of course ultimately means it
*should* be. In all seriousness though, it wouldn't be too crazy to
plug something like [dhall](https://dhall-lang.org/) in here.

It also served as motivation to start
[EasyFuse](https://gitlab.com/jD91mZM2/easyfuse), although
unfortunately it looks like it won't hold as motivation to finish it.
