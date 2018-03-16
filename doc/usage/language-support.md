# Language support

* [Java](#java)
  * [Java keywords](#java-keywords)
  * [`jackson` module](#modulesjackson)
  * [`lombok` module](#moduleslombok)
  * [`builder` module](#modulesbuilder)
* [Rust](#rust)
  * [Rust keywords](#rust-keywords)
  * [`chrono` module](#moduleschrono)
* [Python](#python)
  * [Python keywords](#python-keywords)
* [JavaScript](#javascript)
  * [JavaScript keywords](#javascript-keywords)
* [C#](#csharp)
  * [`Json.NET` module](#modulesjsonnet)
* [Swift](#swift)
  * [`codable` module](#modulescodable)
  * [`simple` module](#modulessimple)
* [Go](#go)
  * [`encoding/json` module](#modulesencodingjson)

This section details the how each language behaves, and which modules and options are available to
tweak this behavior.

The primary goal of `reproto` is to integrate with idiomatic language-specific ways to do
serialization.
Because each language is different, these bindings can differ significantly.

Every language also has a number of _keywords_ and other restrictions, making the task of
generating code for them interesting.

For example, using a field named `class`, like this:

```reproto
type Example {
  class: string;
}
```

Would result in the field name being escaped in language that has this keyword, like in Java:

```
public class Example {
  private final String _class;

  public String getClass_() {
    return this._class;
  }
}
```

Find the relevant section for your language to find out how this is done.

## Java

Java is a statically typed language that has a number of framework avaialble to do JSON
serialization.

Classes are generated using _nested_ classes that matches the hierarchy specified in the
specification.

This means that the following specification:

```reproto
// file: src/io/reproto/example.reproto

type Foo {
  // skipped

  type Bar {
    // skipped
  }
}
```

Would result in the following Java classes:

```java
// File: target/io/reproto/example/Foo.java

package io.reproto.example;

public class Foo {
  // skipped

  public static class Bar {
    // skipped
  }
}
```

### Java keywords

Fields which matches keywords of the language will be prefixed with `_`.

The accessor for any field named `class` will be `getClass_` (ends with underscore) to avoid
conflicting with the implicitly defined `Object#getClass`.

### `[modules.jackson]`

```toml
# reproto.toml

[modules.jackson]
```

Adds [Jackson] annotations to generated classes and generates support classes for handling tuples.

Due to the rather sparse default behavior provided by Jackson, there are a number of modules and
options that should be set on the `ObjectMapper` to provide the correct implementation.
These are shown in the example below.

The following is a complete example using Jackson:

```java
import com.fasterxml.jackson.annotation.JsonInclude.Include;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.datatype.jdk8.Jdk8Module;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import test.Entry;

public class Test {
  public static void main(String[] argv) throws Exception {
    final ObjectMapper m = new ObjectMapper();
    // We explicitly support empty "beans"
    m.disable(SerializationFeature.FAIL_ON_EMPTY_BEANS);
    // Do not serialize absent values at all.
    m.setSerializationInclusion(Include.NON_ABSENT);
    // Include support for Optional.
    m.registerModule(new Jdk8Module());
    // Include support for Instant.
    m.registerModule(new JavaTimeModule());

    final BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));

    while (true) {
      final String line = reader.readLine();

      if (line == null) {
        break;
      }

      final Entry entry = m.readValue(line, Entry.class);
      System.out.println(m.writeValueAsString(entry));
    }
  }
}
```

[jackson]: https://github.com/FasterXML/jackson

### `[modules.lombok]`

```toml
# reproto.toml

language = "java"
paths = ["src"]

[modules.lombok]

[packages]
"io.reproto.example" = "*"
```

Adds [lombok] annotations to generated classes.

[lombok]: https://projectlombok.org

### `[modules.builder]`

```toml
# reproto.toml

language = "java"
paths = ["src"]

[modules.builder]

[packages]
"io.reproto.example" = "*"
```

Generates builders for all classes.

The following:

```reproto
// File: src/io/reproto/examples.reproto

type Foo {
  field: string;
}
```

Would generate:

```java
package io.reproto.examples;

public class Foo {
  // skipped

  public static class Builder {
    // skipped

    public Builder field(final String field) {
      // skipped
    }

    public Foo build() {
      // skipped
    }
  }
}
```

## Rust

```toml
# reproto.toml

language = "rust"
paths = ["src"]
output = "target"

[packages]
"io.reproto.example" = "*"
```

Code generation for rust relies entirely on [Serde].

You'll need to add the following dependencies to your project:

```toml
[dependencies]
serde_json = "1"
serde = "1"
serde_derive = "1"
```

And the following extern declarations:

```rust
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
```

Rust does not support nested structs, so generated types follow a naming strategy like the
following:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  // skipped

  type Bar {
    // skipped
  }
}
```

```rust
// File: target/io/reproto/example.rs

struct Foo {
  // skipped
}

struct Foo_Bar {
  // skipped
}
```

[Serde]: https://serde.rs

### Rust keywords

Fields which matches keywords of the language will be prefixed with `_`.

For example:

```reproto
type Entry {
  trait: string;
  _true: string;
}
```

Results in the following Rust `struct`:

```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
  #[serde(rename = "trait")]
  _trait: String,
  #[serde(rename = "true")]
  _true: String,
}
```

### `[modules.chrono]`

```toml
# reproto.toml

language = "rust"
paths = ["src"]

[modules.chrono]

[packages]
"io.reproto.example" = "*"
```

Rust doesn't have a native type to represent `datetime`, so the `chrono` module is used to
support that through the [`chrono` crate].

You'll need to add the following dependency to your `Cargo.toml`:

```toml
[dependencies]
chrono = {version = "0.4", features = ["serde"]}
```

[`chrono` crate]: https://crates.io/crates/chrono

## Python

```toml
# File: reproto.toml

language = "python"
paths = ["src"]
output = "target"

[packages]
"io.reproto.example" = "*"
```

In python, generated types follow a naming strategy like the following:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  // skipped

  type Bar {
    // skipped
  }
}
```

```python
# File: target/io/reproto/example.py

class Foo:
  pass

class Foo_Bar:
  pass
```

### Python keywords

Fields which matches keywords of the language will be prefixed with `_`.

For example:

```reproto
type Entry {
  import: string;
  print: string;
}
```

Results in the following Python class:

```python
class Entry:
  def __init__(self, _import, _print):
    self._import = _import
    self._print = _print

  @staticmethod
  def decode(data):
    if "import" in data:
      f_import = data["import"]

      if f_import is not None:
        f_import = f_import
    else:
      f_import = None

    if "print" in data:
      f_print = data["print"]

      if f_print is not None:
        f_print = f_print
    else:
      f_print = None

    return Entry(f_import, f_print)

  def encode(self):
    if self._import is not None:
      data["import"] = self._import

    if self._print is not None:
      data["print"] = self._print

    return data

  def __repr__(self):
    return "<Entry import: {!r}, print: {!r}>".format(self._import, self._print)
```

## JavaScript

```toml
# File: reproto.toml

language = "js"
paths = ["src"]
output = "target"

[packages]
"io.reproto.example" = "*"
```

In JavaScript, generated types follow a naming strategy like the following:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  // skipped

  type Bar {
    // skipped
  }
}
```

```javascript
// File: target/io/reproto/example.js

class Foo {
  // skipped
}

class Foo_Bar {
  // skipped
}
```

### JavaScript keywords

Fields which matches keywords of the language will be prefixed with `_`.

For example:

```reproto
type Entry {
  abstract: string;
  true: string;
}
```

Results in the following JavaScript class:

```javascript
export class Entry {
  constructor(_abstract, _true) {
    this._abstract = _abstract;
    this._true = _true;
  }

  static decode(data) {
    let v_abstract = data["abstract"];

    if (v_abstract !== null && v_abstract !== undefined) {
      v_abstract = v_abstract;
    } else {
      v_abstract = null;
    }

    let v_true = data["true"];

    if (v_true !== null && v_true !== undefined) {
      v_true = v_true;
    } else {
      v_true = null;
    }

    return new Entry(v_abstract, v_true);
  }

  encode() {
    const data = {};

    if (this._abstract !== null && this._abstract !== undefined) {
      data["abstract"] = this._abstract;
    }

    if (this._true !== null && this._true !== undefined) {
      data["true"] = this._true;
    }

    return data;
  }
}
```

## <a id="csharp"></a>C#

```toml
# File: reproto.toml

language = "csharp"
paths = ["src"]

[modules."Json.NET"]

[packages]
"io.reproto.example" = "*"
```

In C#, generated types follow a naming strategy like the following:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  name: string;

  type Bar {
    // skipped
  }
}
```

```cs
// File: Io/Reproto/Example/Foo.cs

namespace Io.Reproto.Example {
  class Foo {
    // skipped
  }

  class Foo_Bar {
    // skipped
  }
}
```

In order to use the generated modules there are two required dependencies that can be installed
with `dotnet`:

```bash
dotnet add package Newtonsoft.Json
dotnet add package JsonSubTypes
```

After this, you can use the models like this:

```cs
using System;
using Newtonsoft.Json;
using Io.Reproto.Example;

namespace Reproto
{
    class Program
    {
        static void Main(string[] args)
        {
            string line = "{\"name\": \"world\"}";
            Foo foo = JsonConvert.DeserializeObject<Foo>(line);
            Console.WriteLine(JsonConvert.SerializeObject(foo));
        }
    }
}
```

### `[modules."Json.NET"]`

```toml
# File: reproto.toml

[modules."Json.NET"]
```

This provides [`Json.NET`] (a.k.a. `Newtonsoft.Json`) annotations and serializers for all types.

Since `Json.NET` doesn't natively support polymorhic sub-typing with custom fields, interfaces are
supported through the [`JsonSubTypes`] project.

The following is a complete example using the `Json.NET` module:

```cs
using System;
using Newtonsoft.Json;

namespace Reproto
{
    class Program
    {
        static void Main(string[] args)
        {
            string line;

            while ((line = Console.ReadLine()) != null) {
                Test.Entry foo = JsonConvert.DeserializeObject<Test.Entry>(line);
                Console.WriteLine(JsonConvert.SerializeObject(foo));
            }
        }
    }
}
```

[`Json.NET`]: https://www.newtonsoft.com/json
[`JsonSubTypes`]: https://github.com/manuc66/JsonSubTypes

## Swift

```toml
# File: reproto.toml

language = "swift"

[modules.codable]

[presets.swift]

[packages]
"io.reproto.example" = "*"
```

In Swift, generated types follow a naming strategy like the following:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  name: string;

  type Bar {
    // skipped
  }
}
```

```swift
// File: Models/Io/Reproto/Example.swift

public struct Io_Reproto_Example_Foo {
  // skipped
}

public extension Io_Reproto_Example_Foo {
  static func decode(json: Any) throws -> Io_Reproto_Example_Foo;
}

public struct Io_Reproto_Example_Foo_Bar {
  // skipped
}
```

### `[modules.codable]`

```toml
# reproto.toml

[modules.codable]
```

This module uses the [`Codable`] framework to annotate types.

Since there is no implementation for `Any`, this is provided through an `AnyCodable` shim.
Therefore, the codable module is _not_ compatible with other serialization methods.

It is also required to set the serialization options for `Date` to be ISO-8601.

```swift
import Foundation
import Models

let decoder = JSONDecoder()
decoder.dateDecodingStrategy = .iso8601

let encoder = JSONEncoder()
encoder.dateEncodingStrategy = .iso8601

while let line = readLine() {
    let json = line.data(using: String.Encoding.utf8)!
    let entry = try decoder.decode(Io_Reproto_Example_Foo.self, from: json)
    let data = try encoder.encode(entry)
    let out = String(data: data, encoding: String.Encoding.utf8) as String!
    print(out!)
}

```

[`Codable`]: https://developer.apple.com/documentation/swift/codable

#### `ReprotoCodable_Utils.swift`

This is a helper generated by the `codable` module.

This provides the `AnyCodable`, and `AnyNull` types which are required since [`Codable`] does not
support `Any`.

[`Codable`]: https://developer.apple.com/documentation/swift/codable

### `[modules.simple]`

```toml
# reproto.toml

[modules.simple]
```

This module generates `decode` and `encode` methods for each type.
These provide implementations of the encoding and decoding mechanisms necessary to bind
deserialized JSON to the struct.

The following is an example of how these can be used:

```swift
import Foundation;
import Models;

let data = "{\"name\": \"world\"}"

let json = try? JSONSerialization.jsonObject(with: data.data(using: String.Encoding.utf8)!)
let entry = try Io_Reproto_Example_Foo.decode(json: json as! [String: Any])
let data = try JSONSerialization.data(withJSONObject: entry.encode())
let out = String(data: data, encoding: String.Encoding.utf8) as String!
```

#### `ReprotoSimple_Utils.swift`

This is a support file that is generated for the `simple` module.

It contains a number of module-private helper functions needed to drive serialization.

```swift
// File: Sources/Models/ReprotoUtils.swift

enum SerializationError: Error {
  case missing(String)
  case invalid(String)
  case bad_value()
}

func decode_name<T>(_ unbox: T?, name string: String) throws -> T;

func decode_value<T>(_ value: T?) throws -> T;

func unbox(_ value: Any, as type: Int.Type) -> Int?;

func unbox(_ value: Any, as type: UInt.Type) -> UInt?;

func unbox(_ value: Any, as type: Int32.Type) -> Int32?;

func unbox(_ value: Any, as type: Int64.Type) -> Int64?;

func unbox(_ value: Any, as type: UInt32.Type) -> UInt32?;

func unbox(_ value: Any, as type: UInt64.Type) -> UInt64?;

func unbox(_ value: Any, as type: Float.Type) -> Float?;

func unbox(_ value: Any, as type: Double.Type) -> Double?;

func unbox(_ value: Any, as type: String.Type) -> String?;

func unbox(_ value: Any, as type: Bool.Type) -> Bool?;

func decode_array<T>(_ value: Any, name: String, inner: (Any) throws -> T) throws -> [T];

func encode_array<T>(_ array: [T], name: String, inner: (T) throws -> Any) throws -> [Any];

func decode_map<T>(_ map: Any, name: String, value: (Any) throws -> T) throws -> [String: T];

func encode_map<T>(_ map: [String: T], name: String, value: (T) throws -> Any) throws -> [String: Any];
```

## Go

```toml
# File: reproto.toml

language = "go"

[modules."encoding/json"]

[presets.go]

[packages]
"io.reproto.example" = "*"
```

In Go it is recommended to use the [`go` preset], and import the models relative to your project:

```go
package main

import "./models/io_reproto_example"

func main() {
    // do something with io_reproto_example.Foo
}
```

Generated types are put into individual modules corresponding to their package, and the type
hierarchy is flattened.

For example:

```reproto
// File: src/io/reproto/example.reproto

type Foo {
  name: string;

  type Bar {
    // skipped
  }
}
```

Becomes:

```swift
// File: models/io_reproto_example.go
package io_reproto_example

type Foo struct {
  // skipped
}

type Foo_Bar struct {
  // skipped
}
```

[`go` preset]: ../manifest.md#go-preset

### `[modules."encoding/json"]`

```toml
# reproto.toml

[modules."encoding/json"]
```

This module makes use of Go's built-in [`encoding/json`] module.

Types use the existing struct serialization, optional fields are pointers (`*`) with the
`omitempty` option set.
Tuples are also structs, but have custom marshal/unmarshal implementations.
Enums are encoded as type aliases with a number of constants corresponding to the variants.

[`encoding/json`]: https://golang.org/pkg/encoding/json/
