// Stage 23: multiple top-level classes – instance + static methods.
// Counter is a package-private helper class with both instance and static
// methods; MultiClassInstance uses it from the public driver class.
class Counter {
    int count;

    Counter() {
        this.count = 0;
    }

    void increment() {
        this.count++;
    }

    int value() {
        return this.count;
    }

    static Counter create() {
        return new Counter();
    }
}

public class MultiClassInstance {
    public static void main(String[] args) {
        Counter c1 = Counter.create();
        Counter c2 = new Counter();
        c1.increment();
        c1.increment();
        c2.increment();
        System.out.println(c1.value());
        System.out.println(c2.value());
    }
}
