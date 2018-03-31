package test;

import com.fasterxml.jackson.annotation.JsonCreator;

public class Entry {
  @JsonCreator
  public Entry() {
  }

  @Override
  public int hashCode() {
    int result = 1;
    return result;
  }

  @Override
  public boolean equals(final Object other) {
    if (other == null) {
      return false;
    }

    if (!(other instanceof Entry)) {
      return false;
    }

    @SuppressWarnings("unchecked")
    final Entry o = (Entry) other;

    return true;
  }

  @Override
  public String toString() {
    final StringBuilder b = new StringBuilder();

    b.append("Entry");
    b.append("(");
    b.append(")");

    return b.toString();
  }

  public static class Builder {
    public Entry build() {

      return new Entry();
    }
  }
}
