public class Power {
    public static long power(long base, int exp) {
        long result = 1;
        for (int i = 0; i < exp; i++) {
            result *= base;
        }
        return result;
    }
    public static void main(String[] args) {
        System.out.println(power(2, 10));
        System.out.println(power(3, 5));
    }
}
