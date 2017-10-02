package test;

import java.util.Objects;

public enum Entry {
  A("foo"),
  B("bar");

  private final String value;

  private Entry(
    final String value
  ) {
    Objects.requireNonNull(value, "value");
    this.value = value;
  }

  public static Entry fromValue(final String value) {
    for (final Entry value : values()) {
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
