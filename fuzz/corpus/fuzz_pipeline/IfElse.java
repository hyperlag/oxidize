public class IfElse {
    public static void main(String[] args) {
        int x = 7;
        if (x > 5) {
            System.out.println("big");
        } else {
            System.out.println("small");
        }
        if (x < 0) {
            System.out.println("negative");
        } else if (x == 0) {
            System.out.println("zero");
        } else {
            System.out.println("positive");
        }
    }
}
