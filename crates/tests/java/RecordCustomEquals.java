record Point(int x, int y) {
    // Custom equals: two points are equal if they have the same magnitude
    boolean equals(Point other) {
        return (x * x + y * y) == (other.x() * other.x() + other.y() * other.y());
    }

    public int hashCode() {
        return x * x + y * y;
    }

    public String toString() {
        return "(" + x + ", " + y + ")";
    }
}

class RecordCustomEquals {
    public static void main(String[] args) {
        Point a = new Point(3, 4);
        Point b = new Point(4, 3);
        Point c = new Point(1, 2);

        // Custom equals: same magnitude (3^2+4^2 == 4^2+3^2 == 25)
        System.out.println(a.equals(b));   // true
        System.out.println(a.equals(c));   // false

        // Custom hashCode
        System.out.println(a.hashCode());  // 25

        // Custom toString
        System.out.println(a.toString());  // (3, 4)

        // Record Display (should use record format since toString is a separate method)
        System.out.println(a);  // Point[x=3, y=4]
    }
}
