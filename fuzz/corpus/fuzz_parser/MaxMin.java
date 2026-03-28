public class MaxMin {
    public static int max(int a, int b) {
        return a > b ? a : b;
    }
    public static int min(int a, int b) {
        return a < b ? a : b;
    }
    public static void main(String[] args) {
        System.out.println(max(3, 7));
        System.out.println(min(3, 7));
        System.out.println(max(-1, -5));
    }
}
