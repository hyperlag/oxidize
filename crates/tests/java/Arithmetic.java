public class Arithmetic {
    public static int add(int a, int b) {
        return a + b;
    }

    public static int multiply(int a, int b) {
        return a * b;
    }

    public static double divide(double a, double b) {
        return a / b;
    }

    public static int remainder(int a, int b) {
        return a % b;
    }

    public static void main(String[] args) {
        System.out.println(add(3, 4));
        System.out.println(multiply(6, 7));
        System.out.println(remainder(10, 3));
    }
}
