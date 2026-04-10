import java.util.Arrays;

public class ArraysSort {
    public static void main(String[] args) {
        int[] arr = new int[5];
        arr[0] = 5;
        arr[1] = 3;
        arr[2] = 1;
        arr[3] = 4;
        arr[4] = 2;
        Arrays.sort(arr);
        System.out.println(arr[0]);
        System.out.println(arr[4]);

        int idx = Arrays.binarySearch(arr, 4);
        System.out.println(idx >= 0);

        int[] range = Arrays.copyOfRange(arr, 1, 3);
        System.out.println(range[0]);
        System.out.println(range.length);
    }
}
