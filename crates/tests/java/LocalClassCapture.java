class LocalClassCapture {
    public static void main(String[] args) {
        int offset = 100;
        String label = "result";

        class Adder {
            int add(int x) {
                return offset + x;
            }
            void print(int x) {
                System.out.println(label + "=" + add(x));
            }
        }

        Adder a = new Adder();
        System.out.println(a.add(5));
        System.out.println(a.add(42));
        a.print(7);
    }
}
