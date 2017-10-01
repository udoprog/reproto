export class Entry {
  constructor(ordinal, name, field) {
    this.ordinal = ordinal;
    this.name = name;
    this.field = field;
  }

  encode() {
    return this.ordinal;
  }
  static decode(data) {
    for (let i = 0, l = Entry.values.length; i < l; i++) {
      const member = Entry.values[i]



      if (member.ordinal === data) {
        return member;
      }
    }

    throw new Error("no matching value");
  }
}

Entry.A = new Entry(0, "A", "foo");
Entry.B = new Entry(1, "B", "bar");

Entry.values = [Entry.A, Entry.B];
