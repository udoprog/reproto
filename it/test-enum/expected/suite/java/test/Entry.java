package test;

import java.util.Objects;

public enum Entry {
  A("foo"),
  B("bar");

  private final String field;

  private Entry(
    final String field
  ) {
    Objects.requireNonNull(field, "field");
    this.field = field;
  }

  public String getField() {
    return this.field;
  }
}
