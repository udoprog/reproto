* [Getting started](#getting-started)
* [Rebuilding project on changes](#rebuilding-project-on-changes)

# Getting started

Reproto can be installed through cargo:

```
$ cargo install reproto
```

To easily get started with reproto, you can initialize a new project using `reproto init`.
This will create a basic [`reproto.toml`], which can be used to customize how your
specifications are built.

Let's start by setting up a simple specification in an isolated directory:

```bash
$ mkdir example
$ cd example
$ reproto init
INFO - Writing Manifest: reproto.toml
INFO - Creating: proto/io/reproto
INFO - Writing: proto/io/reproto/example.reproto
```

Next, let's try to compile this project into Java using a couple of modules:

```bash
$ reproto --debug build --lang java --module jackson --module lombok
```

You should now have a number of files generated in `target/io/reproto/example`, corresponding to
the schema that is defined in `proto/example.reproto`.

Next up, you might be interested to read the following sections:

* Documentation for the [specification language].
* Documentation for the [build manifest].

[specification language]: ../spec.md
[build manifest]: ../manifest.md
[`reproto.toml`]: ../manifest.md

## Rebuilding project on changes

Reproto supports watching a project through `reproto watch`.

Any changes done to any specification or to any relevant manifest files will cause the project to
be rebuilt automatically.

```bash
$ reproto watch
INFO - Project Built!
```

This is useful when using reproto with an IDE that doesn't have native support for reproto
projects.

You can keep reproto running in the background as you are editing your manifests, and the IDE
should automatically pick up any updated files.
