import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

class CollectionsSort {
    public static void main(String[] args) {
        List<Integer> nums = new ArrayList<>();
        nums.add(30);
        nums.add(10);
        nums.add(20);
        nums.add(5);
        Collections.sort(nums);
        for (int n : nums) {
            System.out.println(n);
        }
        Collections.reverse(nums);
        for (int n : nums) {
            System.out.println(n);
        }
    }
}
