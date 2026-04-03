interface Describable {
    String describe();
}

public class EnumInterface {
    enum Shape implements Describable {
        CIRCLE,
        SQUARE,
        TRIANGLE;

        public String describe() {
            return "Shape:" + this.name();
        }
    }

    public static void main(String[] args) {
        System.out.println(Shape.CIRCLE.describe());
        System.out.println(Shape.SQUARE.describe());
        System.out.println(Shape.TRIANGLE.describe());
    }
}
