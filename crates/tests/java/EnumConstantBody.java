public class EnumConstantBody {
    enum Operation {
        ADD {
            public int apply(int a, int b) { return a + b; }
        },
        SUBTRACT {
            public int apply(int a, int b) { return a - b; }
        },
        MULTIPLY {
            public int apply(int a, int b) { return a * b; }
        };

        public int apply(int a, int b) { return 0; }
    }

    public static void main(String[] args) {
        System.out.println(Operation.ADD.apply(3, 4));
        System.out.println(Operation.SUBTRACT.apply(10, 3));
        System.out.println(Operation.MULTIPLY.apply(5, 6));
    }
}
