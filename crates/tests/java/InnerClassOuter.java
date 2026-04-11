// Tests Stage 12D: non-static inner class accessing outer class fields and
// methods via the explicit `OuterClass.this.field` qualifier.
class InnerClassOuter {
    int value;
    String label;

    InnerClassOuter() {
        value = 5;
        label = "data";
    }

    class Printer {
        void print() {
            System.out.println(InnerClassOuter.this.label + "=" + InnerClassOuter.this.value);
            InnerClassOuter.this.helper();
        }
    }

    class Calculator {
        int doubled() {
            return InnerClassOuter.this.value * 2;
        }
    }

    void helper() {
        System.out.println("helper called");
    }

    void run() {
        Printer p = new Printer();
        p.print();
        Calculator c = new Calculator();
        System.out.println("doubled=" + c.doubled());
    }

    public static void main(String[] args) {
        InnerClassOuter obj = new InnerClassOuter();
        obj.run();
    }
}
