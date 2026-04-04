public class LabeledBreak {
    public static void main(String[] args) {
        // Labeled break: stop outer loop when inner hits target cell.
        outer:
        for (int i = 0; i < 4; i++) {
            for (int j = 0; j < 4; j++) {
                if (i == 2 && j == 2) {
                    break outer;
                }
                System.out.println(i + "," + j);
            }
        }
        System.out.println("done");
    }
}
