record Point(int x, int y) {
    String describe() {
        return "(" + x + "," + y + ")";
    }
}

class RecordBasic {
    public static void main(String[] args) {
        Point p = new Point(3, 4);
        System.out.println(p.x());
        System.out.println(p.y());
        System.out.println(p);
        System.out.println(p.describe());
    }
}
