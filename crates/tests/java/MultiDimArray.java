public class MultiDimArray {
    public static void main(String[] args) {
        int[][] matrix = new int[3][4];
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 4; j++) {
                matrix[i][j] = i * 4 + j;
            }
        }
        System.out.println(matrix[0][0]);
        System.out.println(matrix[1][2]);
        System.out.println(matrix[2][3]);

        // 2D boolean array
        boolean[][] flags = new boolean[2][3];
        flags[0][1] = true;
        flags[1][2] = true;
        System.out.println(flags[0][0]);
        System.out.println(flags[0][1]);
        System.out.println(flags[1][2]);
    }
}
