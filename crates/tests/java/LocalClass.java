class LocalClass {
    public static void main(String[] args) {
        class Point {
            int x;
            int y;

            Point(int x, int y) {
                this.x = x;
                this.y = y;
            }

            void print() {
                System.out.println(x + "," + y);
            }
        }

        Point p = new Point(3, 4);
        p.print();
        System.out.println("done");
    }
}
