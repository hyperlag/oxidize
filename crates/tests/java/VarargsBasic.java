public class VarargsBasic {
    public static int sum(int... nums) {
        int total = 0;
        for (int i = 0; i < nums.length; i++) {
            total += nums[i];
        }
        return total;
    }

    public static int max(int first, int... rest) {
        int m = first;
        for (int i = 0; i < rest.length; i++) {
            if (rest[i] > m) m = rest[i];
        }
        return m;
    }

    public static void main(String[] args) {
        System.out.println(sum(1, 2, 3));
        System.out.println(sum(10, 20));
        System.out.println(sum());
        System.out.println(max(5, 3, 8, 1));
    }
}
