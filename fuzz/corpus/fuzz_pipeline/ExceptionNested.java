public class ExceptionNested {
    public static void main(String[] args) {
        try {
            try {
                throw new RuntimeException("inner");
            } finally {
                System.out.println("inner finally");
            }
        } catch (RuntimeException e) {
            System.out.println("outer catch: " + e.getMessage());
        }
        System.out.println("done");
    }
}
