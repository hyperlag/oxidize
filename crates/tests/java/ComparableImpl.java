import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.TreeSet;

class Student implements Comparable<Student> {
    String name;
    int grade;

    Student(String name, int grade) {
        this.name = name;
        this.grade = grade;
    }

    public int compareTo(Student other) {
        return this.name.compareTo(other.name);
    }

    public String toString() {
        return name + "(" + grade + ")";
    }
}

class ComparableImpl {
    public static void main(String[] args) {
        List<Student> students = new ArrayList<>();
        students.add(new Student("Charlie", 85));
        students.add(new Student("Alice", 92));
        students.add(new Student("Bob", 78));

        Collections.sort(students);

        for (Student s : students) {
            System.out.println(s);
        }

        TreeSet<Student> set = new TreeSet<>();
        set.add(new Student("Zara", 95));
        set.add(new Student("Adam", 88));
        set.add(new Student("Maya", 91));

        for (Student s : set) {
            System.out.println(s.name);
        }
    }
}
