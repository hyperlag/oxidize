import java.util.Arrays;

public class ArraysFill {
    public static void main(String[] args) {
        int[] arr = new int[4];
        Arrays.fill(arr, 7);
        System.out.println(arr[0]);
        System.out.println(arr[3]);

        int[] copy = Arrays.copyOf(arr, 6);
        System.out.println(copy[0]);
        System.out.println(copy[4]);
        System.out.println(copy.length);
    }
}
