sealed interface Shape permits Circle, Rect {
    int perimeter();
}

final class Circle implements Shape {
    public int perimeter() {
        return 10;
    }
}

final class Rect implements Shape {
    public int perimeter() {
        return 14;
    }
}

class SealedClass {
    public static void main(String[] args) {
        Circle c = new Circle();
        Rect r = new Rect();
        System.out.println(c.perimeter());
        System.out.println(r.perimeter());
    }
}
