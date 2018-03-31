#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RootType {
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RootInterface {
  Foo {
  },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RootEnum {
  Foo,
}

impl RootEnum {
  pub fn value(&self) -> &'static str {
    use self::RootEnum::*;
    match *self {
      Foo => "Foo",
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
struct RootTuple();

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootType_NestedType {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RootType_NestedInterface {
  Foo {
  },
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub enum RootType_NestedEnum {
  Foo,
}

impl RootType_NestedEnum {
  pub fn value(&self) -> &'static str {
    use self::RootType_NestedEnum::*;
    match *self {
      Foo => "Foo",
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
struct RootType_NestedTuple();

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootInterface_Foo_NestedType {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RootInterface_Foo_NestedInterface {
  NestedFoo {
  },
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub enum RootInterface_Foo_NestedEnum {
  Foo,
}

impl RootInterface_Foo_NestedEnum {
  pub fn value(&self) -> &'static str {
    use self::RootInterface_Foo_NestedEnum::*;
    match *self {
      Foo => "Foo",
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
struct RootInterface_Foo_NestedTuple();

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootTuple_NestedType {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RootTuple_NestedInterface {
  Foo {
  },
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub enum RootTuple_NestedEnum {
  Foo,
}

impl RootTuple_NestedEnum {
  pub fn value(&self) -> &'static str {
    use self::RootTuple_NestedEnum::*;
    match *self {
      Foo => "Foo",
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
struct RootTuple_NestedTuple();

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootService_NestedType {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RootService_NestedInterface {
  Foo {
  },
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub enum RootService_NestedEnum {
  Foo,
}

impl RootService_NestedEnum {
  pub fn value(&self) -> &'static str {
    use self::RootService_NestedEnum::*;
    match *self {
      Foo => "Foo",
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
struct RootService_NestedTuple();

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootType_NestedInterface_Foo_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootType_NestedTuple_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootType_NestedService_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootInterface_Foo_NestedInterface_NestedFoo_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootInterface_Foo_NestedTuple_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootInterface_Foo_NestedService_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootTuple_NestedInterface_Foo_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootTuple_NestedTuple_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootTuple_NestedService_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootService_NestedInterface_Foo_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootService_NestedTuple_Nested {
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RootService_NestedService_Nested {
}
