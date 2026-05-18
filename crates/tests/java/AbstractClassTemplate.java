abstract class Shape {
    String name;
    Shape(String name) { this.name = name; }
    abstract double area();
    // Concrete method calling the abstract method: tests that this compiles
    // even in the abstract class.  Not invoked through the abstract reference
    // (virtual dispatch would need the override), but through concrete refs.
    String label() { return name; }
}

class Circle extends Shape {
    double radius;
    Circle(double radius) {
        super("Circle");
        this.radius = radius;
    }
    @Override
    double area() { return 3.14159 * radius * radius; }
    String describe() { return label() + " area=" + area(); }
}

class Rectangle extends Shape {
    double w, h;
    Rectangle(double w, double h) {
        super("Rectangle");
        this.w = w;
        this.h = h;
    }
    @Override
    double area() { return w * h; }
    String describe() { return label() + " area=" + area(); }
}

public class AbstractClassTemplate {
    public static void main(String[] args) {
        Circle c = new Circle(5.0);
        Rectangle r = new Rectangle(3.5, 2.5);
        System.out.println(c.area());            // 78.53975
        System.out.println(r.area());            // 8.75
        System.out.println(c.area() > r.area()); // true
        System.out.println(c.describe());        // Circle area=78.53975
        System.out.println(r.describe());        // Rectangle area=8.75
    }
}
