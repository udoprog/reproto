export class Entry {
  constructor(ordinal, name, value) {
    this.ordinal = ordinal;
    this.name = name;
    this.value = value;
  }

  encode() {
    return this.value;
  }
  static decode(data) {
    for (let i = 0, l = Entry.values.length; i < l; i++) {
      const member = Entry.values[i]



      if (member.value === data) {
        return member;
      }
    }

    throw new Error("no matching value");
  }
}

Entry.A = new Entry("foo", "A");
Entry.B = new Entry("bar", "B");

Entry.values = [Entry.A, Entry.B];

export class Entry2 {
  constructor(ordinal, name, value) {
    this.ordinal = ordinal;
    this.name = name;
    this.value = value;
  }

  encode() {
    return this.value;
  }
  static decode(data) {
    for (let i = 0, l = Entry2.values.length; i < l; i++) {
      const member = Entry2.values[i]



      if (member.value === data) {
        return member;
      }
    }

    throw new Error("no matching value");
  }
}

Entry2.A = new Entry2("A", "A");
Entry2.B = new Entry2("B", "B");
Entry2.C = new Entry2("C", "C");

Entry2.values = [Entry2.A, Entry2.B, Entry2.C];
