package test;

import java.util.Objects;

public enum Entry2 {
  A("A"),
  B("B"),
  C("C");

  private final String value;

  private Entry2(
    final String value
  ) {
    Objects.requireNonNull(value, "value");
    this.value = value;
  }

  public static Entry2 fromValue(final String value) {
    for (final Entry2 value : values()) {
      if (value.value.equals(value)) {
        return value;
      }
    }

    throw new IllegalArgumentException("value");
  }

  public String toValue() {
    return this.value;
  }
}
