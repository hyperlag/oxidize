public class ControlFlow {
    public static int factorial(int n) {
        int result = 1;
        for (int i = 2; i <= n; i++) {
            result = result * i;
        }
        return result;
    }

    public static int fibonacci(int n) {
        if (n <= 1) {
            return n;
        }
        int a = 0;
        int b = 1;
        int i = 2;
        while (i <= n) {
            int tmp = a + b;
            a = b;
            b = tmp;
            i++;
        }
        return b;
    }

    public static String classify(int n) {
        if (n < 0) {
            return "negative";
        } else if (n == 0) {
            return "zero";
        } else {
            return "positive";
        }
    }

    public static void main(String[] args) {
        System.out.println(factorial(5));
        System.out.println(fibonacci(10));
        System.out.println(classify(-3));
        System.out.println(classify(0));
        System.out.println(classify(7));
    }
}
