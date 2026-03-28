public class GCD {
    public static int gcd(int a, int b) {
        while (b != 0) {
            int t = b;
            b = a % b;
            a = t;
        }
        return a;
    }
    public static void main(String[] args) {
        System.out.println(gcd(48, 18));
        System.out.println(gcd(100, 75));
    }
}
